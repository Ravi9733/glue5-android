package com.ravi9733.rpfexplorer

import java.io.File
import java.io.RandomAccessFile
import java.io.OutputStream
import java.io.FileOutputStream
import java.io.FileInputStream
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.util.ArrayDeque

/**
 * Non-destructive RPF7 OPEN/NONE writer.
 *
 * It rebuilds the TOC/name table and 512-byte payload alignment into a new file.
 * Existing compressed binaries are decoded through RpfArchive.extract and emitted
 * as uncompressed entries. Resource entries are rejected until their RSC7 payload
 * lengths and page flags can be preserved correctly.
 */
internal class RpfEditSession(private val archive: RpfArchive, private val workDir: File) {
    private val replacements = linkedMapOf<Int, File>()
    private val removals = linkedSetOf<Int>()
    private val additions = mutableListOf<Added>()

    private data class Added(val parent: Int, val name: String, val file: File, val resource: Boolean, val systemFlags: Long, val graphicsFlags: Long)

    fun add(parentDirectory: Int, source: File, name: String = source.name) {
        require(parentDirectory in archive.entries.indices && archive.entries[parentDirectory].isDirectory && parentDirectory !in removals) {
            "Choose a valid destination folder"
        }
        require(name.isNotBlank() && name != "." && name != ".." && name.none { it == '/' || it == '\\' || it == '\u0000' }) { "Invalid file name" }
        require(source.isFile) { "Selected source is not a file" }
        val siblings = archive.children(parentDirectory).filter { it.index !in removals }.map { it.name.lowercase() }.toMutableSet()
        additions.filter { it.parent == parentDirectory }.forEach { siblings.add(it.name.lowercase()) }
        require(name.lowercase() !in siblings) { "A file or folder with this name already exists" }
        val copy = File(workDir, "add-${System.nanoTime()}").also { source.copyTo(it, overwrite = true) }
        val meta = readResourceMeta(copy)
        additions += Added(parentDirectory, name, copy, meta != null, meta?.first ?: 0L, meta?.second ?: 0L)
    }

    fun replace(entryIndex: Int, source: File) {
        require(entryIndex in archive.entries.indices) { "Invalid entry" }
        val entry = archive.entries[entryIndex]
        require(!entry.isDirectory) { "Directories cannot be replaced" }
        require(!entry.encrypted || archive.encryptionLabel == "AES") { "NG-encrypted files cannot be replaced without NG key support" }
        require(source.isFile) { "Selected source is not a file" }
        val meta = readResourceMeta(source)
        require(entry.isResource == (meta != null)) {
            if (entry.isResource) "Replace this RSC7 resource with another RSC7 file" else "A binary entry cannot be replaced by an RSC7 resource"
        }
        replacements[entryIndex] = File(workDir, "replace-$entryIndex-${System.nanoTime()}").also {
            source.copyTo(it, overwrite = true)
        }
        removals.remove(entryIndex)
    }

    fun remove(entryIndex: Int) {
        require(entryIndex in archive.entries.indices && entryIndex != 0) { "Cannot remove archive root" }
        val target = archive.entries[entryIndex]
        val removeSet = linkedSetOf(entryIndex)
        if (target.isDirectory) {
            var changed = true
            while (changed) {
                changed = false
                archive.entries.forEach { e ->
                    if (e.parent in removeSet && removeSet.add(e.index)) changed = true
                }
            }
        }
        removals.addAll(removeSet)
        removeSet.forEach { replacements.remove(it) }
        additions.filter { it.parent in removeSet }.toList().forEach {
            it.file.delete()
            additions.remove(it)
        }
    }

    fun dispose() { workDir.deleteRecursively() }
    fun stagedCount(): Int = additions.size + replacements.size + removals.size
    fun isRemoved(entryIndex: Int): Boolean = entryIndex in removals
    fun isReplaced(entryIndex: Int): Boolean = entryIndex in replacements
    fun replacementFile(entryIndex: Int): File? = replacements[entryIndex]
    fun addedNames(parentDirectory: Int): List<String> = additions.filter { it.parent == parentDirectory }.map { it.name }
    fun removeAdded(parentDirectory: Int, name: String) {
        val item = additions.firstOrNull { it.parent == parentDirectory && it.name == name } ?: return
        item.file.delete()
        additions.remove(item)
    }

    fun saveAs(destination: File) {
        require(archive.encryptionLabel == "OPEN" || archive.encryptionLabel == "NONE" || archive.encryptionLabel == "AES") {
            "This archive encryption mode cannot be repacked"
        }
        require(archive.encryptionLabel != "AES" || archive.entries.none { it.isResource && it.index !in removals }) {
            "AES-encrypted RSC7 resource repacking is not enabled; remove those resources or use an OPEN/NONE archive."
        }
        require(archive.encryptionLabel == "AES" || archive.entries.none { it.encrypted && it.index !in removals }) {
            "Encrypted payloads need compatible keys before repacking."
        }

        data class Node(
            val old: RpfEntry?, val name: String, val parentOld: Int?,
            val file: File?, val directory: Boolean, val resource: Boolean,
            val systemFlags: Long, val graphicsFlags: Long
        )
        val nodes = mutableListOf<Node>()
        val childrenByOld = archive.entries.filter { it.index != 0 && it.index !in removals }
            .groupBy { it.parent }.mapValues { (_, v) -> v.sortedBy { it.index } }
        val root = archive.entries.firstOrNull { it.index == 0 && it.isDirectory }
            ?: error("Archive root directory is missing")
        nodes += Node(root, root.name, null, null, true, false, 0L, 0L)

        // Breadth-first order ensures every directory's direct children occupy a
        // contiguous range, as required by the RPF directory entry.
        val oldToNew = mutableMapOf(0 to 0)
        val queue = ArrayDeque<Int>()
        queue.add(0)
        while (queue.isNotEmpty()) {
            val oldParent = queue.removeFirst()
            val parentNew = oldToNew.getValue(oldParent)
            val start = nodes.size
            childrenByOld[oldParent].orEmpty().forEach { e ->
                val f = if (e.isDirectory) null else replacements[e.index]
                val newResourceMeta = if (e.isResource && f != null) readResourceMeta(f) else null
                nodes += Node(e, e.name, oldParent, f, e.isDirectory, e.isResource,
                    newResourceMeta?.first ?: e.systemFlags, newResourceMeta?.second ?: e.graphicsFlags)
                oldToNew[e.index] = nodes.lastIndex
            }
            additions.filter { it.parent == oldParent }.forEach { a ->
                nodes += Node(null, a.name, oldParent, a.file, false, a.resource, a.systemFlags, a.graphicsFlags)
            }
            val directCount = nodes.size - start
            directoryRanges[oldParent] = start to directCount
            childrenByOld[oldParent].orEmpty().filter { it.isDirectory }.forEach { queue.add(it.index) }
        }

        val nameBytes = java.io.ByteArrayOutputStream()
        val nameOffsets = IntArray(nodes.size)
        val sharedNameOffsets = linkedMapOf<String, Int>()
        nodes.forEachIndexed { i, n ->
            val existing = sharedNameOffsets[n.name]
            if (existing != null) {
                nameOffsets[i] = existing
            } else {
                val bytes = n.name.toByteArray(StandardCharsets.UTF_8)
                require(bytes.isNotEmpty() && bytes.none { it == 0.toByte() }) { "Invalid RPF entry name" }
                nameOffsets[i] = nameBytes.size()
                sharedNameOffsets[n.name] = nameOffsets[i]
                nameBytes.write(bytes); nameBytes.write(0)
            }
        }
        val names = nameBytes.toByteArray()
        require(names.size < 0x10000000) { "RPF name table is too large" }
        require(nodes.size <= 2_000_000) { "Too many RPF entries" }

        val tocStart = 16L
        val namesStart = tocStart + nodes.size * 16L
        val payloadStart = align512(namesStart + names.size)
        val payloadFiles = arrayOfNulls<File>(nodes.size)
        val payloadSizes = LongArray(nodes.size)

        // Materialize each ordinary file independently; archive contents are never
        // loaded as a single byte array, keeping peak memory bounded.
        nodes.forEachIndexed { i, n ->
            if (n.directory) return@forEachIndexed
            val staged = n.file
            val tmp = File(workDir, "payload-$i-${System.nanoTime()}")
            if (staged != null) staged.copyTo(tmp, overwrite = true)
            else {
                val old = n.old ?: error("Missing staged file data")
                FileOutputStream(tmp).use { output ->
                    if (n.resource) archive.copyRaw(old, output) else archive.extract(old, output)
                }
            }
            if (n.resource && tmp.length() >= 0xFFFFFFL) patchResourceLargeSize(tmp, tmp.length())
            payloadFiles[i] = tmp
            payloadSizes[i] = tmp.length()
            require(payloadSizes[i] <= 0xFFFFFFFFL) { "RPF7 files cannot exceed 4 GiB" }
        }

        RandomAccessFile(destination, "rw").use { out ->
            out.setLength(0)
            out.writeU32(0x52504637L)
            out.writeU32(nodes.size.toLong())
            out.writeU32(names.size.toLong())
            out.writeU32(if (archive.encryptionLabel == "OPEN") 0x4E45504FL else 0L)
            val offsets = LongArray(nodes.size)
            var cursor = payloadStart
            nodes.forEachIndexed { i, n ->
                if (!n.directory) {
                    cursor = align512(cursor)
                    offsets[i] = cursor
                    cursor += payloadSizes[i]
                }
            }
            nodes.forEachIndexed { i, n ->
                val nameOffset = nameOffsets[i]
                if (n.directory) {
                    require(nameOffset.toLong() <= 0xFFFFFFFFL)
                    val range = directoryRanges[n.old?.index ?: -1] ?: (0 to 0)
                    out.writeU32(nameOffset.toLong())
                    out.writeU32(0x7FFFFF00L)
                    out.writeU32(range.first.toLong())
                    out.writeU32(range.second.toLong())
                } else if (n.resource) {
                    require(nameOffset <= 0xFFFF) { "RPF resource filename table offset exceeds 65535 bytes" }
                    require(offsets[i] / 512L <= 0x7FFFFFL) { "Archive payload offset exceeds RPF7 limit" }
                    val sizeField = if (payloadSizes[i] >= 0xFFFFFFL) 0xFFFFFF else payloadSizes[i].toInt()
                    out.writeU16(nameOffset)
                    out.writeU24(sizeField)
                    out.writeU24((offsets[i] / 512L).toInt() or 0x800000)
                    out.writeU32(n.systemFlags)
                    out.writeU32(n.graphicsFlags)
                } else {
                    require(nameOffset <= 0xFFFF) { "RPF binary filename table offset exceeds 65535 bytes" }
                    require(offsets[i] / 512L <= 0x7FFFFFL) { "Archive payload offset exceeds RPF7 limit" }
                    out.writeU16(nameOffset)
                    out.writeU24(0) // FileSize=0: stored uncompressed
                    out.writeU24((offsets[i] / 512L).toInt())
                    out.writeU32(payloadSizes[i])
                    out.writeU32(0) // no per-file encryption
                }
            }
            out.write(names)
            if (out.filePointer < payloadStart) out.write(ByteArray((payloadStart - out.filePointer).toInt()))
            nodes.forEachIndexed { i, n ->
                if (n.directory) return@forEachIndexed
                val target = offsets[i]
                if (out.filePointer < target) out.write(ByteArray((target - out.filePointer).toInt()))
                FileInputStream(payloadFiles[i]!!).use { input -> input.copyTo(object : OutputStream() {
                    override fun write(b: Int) = out.write(b)
                    override fun write(b: ByteArray, off: Int, len: Int) = out.write(b, off, len)
                }, 1024 * 1024) }
            }
            out.fd.sync()
        }
        try {
            val verified = RpfArchive.open(destination)
            try {
                require(verified.entries.size == nodes.size) { "Saved RPF entry count validation failed" }
                nodes.forEachIndexed { i, node ->
                    val actual = verified.entries[i]
                    require(actual.name == node.name && actual.isDirectory == node.directory && actual.isResource == node.resource) {
                        "Saved RPF TOC validation failed at entry $i"
                    }
                    if (node.directory) {
                        val expectedCount = directoryRanges[node.old?.index ?: -1]?.second ?: 0
                        require(actual.childrenCount == expectedCount) { "Saved RPF directory range validation failed at entry $i" }
                    } else {
                        val expectedFile = payloadFiles[i] ?: error("Missing writer validation payload")
                        val actualFile = File(workDir, "verify-$i-${System.nanoTime()}")
                        FileOutputStream(actualFile).use { output ->
                            if (node.resource) verified.copyRaw(actual, output) else verified.extract(actual, output)
                        }
                        require(sha256(expectedFile) == sha256(actualFile)) { "Saved RPF payload hash validation failed at entry $i" }
                        actualFile.delete()
                    }
                }
            } finally {
                verified.close()
            }
        } catch (e: Exception) {
            destination.delete()
            throw IllegalStateException("Saved archive failed validation: ${e.message}", e)
        }
    }

    private fun sha256(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buffer = ByteArray(64 * 1024)
            while (true) {
                val n = input.read(buffer)
                if (n < 0) break
                digest.update(buffer, 0, n)
            }
        }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }

    private val directoryRanges = mutableMapOf<Int, Pair<Int, Int>>()

    private fun readResourceMeta(file: File): Pair<Long, Long>? {
        if (!file.isFile || file.length() < 16L) return null
        RandomAccessFile(file, "r").use { r ->
            if (readU32(r, 0) != 0x37435352L) return null
            require(file.length() < 0xFFFFFFL) {
                "Adding or replacing RSC7 resources of 16 MiB or larger is blocked because the size sentinel overlaps header metadata. Existing large resources can still be preserved unchanged."
            }
            return readU32(r, 8) to readU32(r, 12)
        }
    }

    private fun patchResourceLargeSize(file: File, size: Long) {
        require(size <= 0xFFFFFFFFL) { "RSC7 resource exceeds 4 GiB" }
        RandomAccessFile(file, "rw").use { r ->
            r.seek(2); r.write((size ushr 24).toInt() and 255)
            r.seek(5); r.write((size ushr 16).toInt() and 255)
            r.seek(7); r.write(size.toInt() and 255)
            r.seek(14); r.write((size ushr 8).toInt() and 255)
        }
    }

    private fun readU32(r: RandomAccessFile, pos: Long): Long {
        r.seek(pos)
        return r.readUnsignedByte().toLong() or (r.readUnsignedByte().toLong() shl 8) or
            (r.readUnsignedByte().toLong() shl 16) or (r.readUnsignedByte().toLong() shl 24)
    }

    private fun align512(v: Long) = (v + 511L) and -512L
    private fun RandomAccessFile.writeU16(v: Int) { write(v and 255); write((v ushr 8) and 255) }
    private fun RandomAccessFile.writeU24(v: Int) { write(v and 255); write((v ushr 8) and 255); write((v ushr 16) and 255) }
    private fun RandomAccessFile.writeU32(v: Long) {
        write((v and 255).toInt()); write(((v ushr 8) and 255).toInt())
        write(((v ushr 16) and 255).toInt()); write(((v ushr 24) and 255).toInt())
    }
}

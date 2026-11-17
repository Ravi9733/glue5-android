package com.ravi9733.rpfexplorer

import java.io.File
import java.io.RandomAccessFile
import java.io.OutputStream
import java.io.FileOutputStream
import java.nio.charset.StandardCharsets
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

    private data class Added(val parent: Int, val name: String, val file: File)

    fun add(parentDirectory: Int, source: File, name: String = source.name) {
        require(parentDirectory in archive.entries.indices && archive.entries[parentDirectory].isDirectory) {
            "Choose a valid destination folder"
        }
        require(name.isNotBlank() && name.none { it == '/' || it == '\\' || it == '\u0000' }) { "Invalid file name" }
        require(source.isFile) { "Selected source is not a file" }
        val siblings = archive.children(parentDirectory).map { it.name.lowercase() }.toMutableSet()
        additions.filter { it.parent == parentDirectory }.forEach { siblings.add(it.name.lowercase()) }
        require(name.lowercase() !in siblings) { "A file or folder with this name already exists" }
        val copy = File(workDir, "add-${System.nanoTime()}").also { source.copyTo(it, overwrite = true) }
        additions += Added(parentDirectory, name, copy)
    }

    fun replace(entryIndex: Int, source: File) {
        require(entryIndex in archive.entries.indices) { "Invalid entry" }
        val entry = archive.entries[entryIndex]
        require(!entry.isDirectory && !entry.isResource) { "Only ordinary binary files can be replaced in this version" }
        require(!entry.encrypted) { "Encrypted files cannot be replaced without the matching GTA V key" }
        require(source.isFile) { "Selected source is not a file" }
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
    }

    fun stagedCount(): Int = additions.size + replacements.size + removals.size

    fun saveAs(destination: File) {
        require(archive.encryptionLabel == "OPEN" || archive.encryptionLabel == "NONE") {
            "Only OPEN/NONE archives can be written"
        }
        require(archive.entries.none { it.isResource }) {
            "This archive contains RSC7 resource entries. Safe resource-preserving repacking is not implemented yet; no output was written."
        }
        require(archive.entries.none { it.encrypted && it.index !in removals }) {
            "Archive contains encrypted payloads. Matching GTA V keys are required before repacking."
        }

        data class Node(
            val old: RpfEntry?, val name: String, val parentOld: Int?,
            val file: File?, val directory: Boolean
        )
        val nodes = mutableListOf<Node>()
        val childrenByOld = archive.entries.filter { it.index != 0 && it.index !in removals }
            .groupBy { it.parent }.mapValues { (_, v) -> v.sortedBy { it.index } }
        val root = archive.entries.firstOrNull { it.index == 0 && it.isDirectory }
            ?: error("Archive root directory is missing")
        nodes += Node(root, root.name, null, null, true)

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
                nodes += Node(e, e.name, oldParent, f, e.isDirectory)
                oldToNew[e.index] = nodes.lastIndex
            }
            additions.filter { it.parent == oldParent }.forEach { a ->
                nodes += Node(null, a.name, oldParent, a.file, false)
            }
            val directCount = nodes.size - start
            directoryRanges[oldParent] = start to directCount
            childrenByOld[oldParent].orEmpty().filter { it.isDirectory }.forEach { queue.add(it.index) }
        }

        val nameBytes = java.io.ByteArrayOutputStream()
        val nameOffsets = IntArray(nodes.size)
        nodes.forEachIndexed { i, n ->
            nameOffsets[i] = nameBytes.size()
            val bytes = n.name.toByteArray(StandardCharsets.UTF_8)
            require(bytes.isNotEmpty() && bytes.none { it == 0.toByte() }) { "Invalid RPF entry name" }
            nameBytes.write(bytes); nameBytes.write(0)
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
                archive.extract(old, FileOutputStream(tmp))
            }
            payloadFiles[i] = tmp
            payloadSizes[i] = tmp.length()
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
                } else {
                    require(nameOffset <= 0xFFFF) { "RPF binary filename table offset exceeds 65535 bytes" }
                    require(offsets[i] / 512L <= 0x7FFFFFL) { "Archive payload offset exceeds RPF7 limit" }
                    require(payloadSizes[i] <= 0xFFFFFFL) { "File exceeds the RPF7 24-bit size limit" }
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
    }

    private val directoryRanges = mutableMapOf<Int, Pair<Int, Int>>()

    private fun align512(v: Long) = (v + 511L) and -512L
    private fun RandomAccessFile.writeU16(v: Int) { write(v and 255); write((v ushr 8) and 255) }
    private fun RandomAccessFile.writeU24(v: Int) { write(v and 255); write((v ushr 8) and 255); write((v ushr 16) and 255) }
    private fun RandomAccessFile.writeU32(v: Long) {
        write((v and 255).toInt()); write(((v ushr 8) and 255).toInt())
        write(((v ushr 16) and 255).toInt()); write(((v ushr 24) and 255).toInt())
    }
}

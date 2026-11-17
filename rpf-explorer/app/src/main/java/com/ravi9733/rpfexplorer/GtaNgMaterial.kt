package com.ravi9733.rpfexplorer

import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.File
import java.io.RandomAccessFile
import java.security.MessageDigest

/**
 * NG material is extracted from the user's own PC GTA5.exe using SHA-1 fingerprints.
 * No game keys or lookup tables are embedded in the application.
 *
 * NG block-decryption follows the MIT-licensed CodeWalker GTACrypto implementation.
 */
internal class GtaNgMaterial private constructor(
    private val keys: Array<ByteArray>,
    private val tables: Array<Array<IntArray>>,
    private val hashLut: ByteArray
) {
    fun decrypt(data: ByteArray, name: String, length: Long): ByteArray {
        require(length in 0..0xFFFFFFFFL) { "NG archive/file length is outside the supported range" }
        val keyIndex = ((calculateHash(name).toLong() and 0xFFFFFFFFL) + length + 61L) % 101L
        val key = keys[keyIndex.toInt()]
        val words = IntArray(68) { i ->
            val o = i * 4
            (key[o].toInt() and 255) or
                ((key[o + 1].toInt() and 255) shl 8) or
                ((key[o + 2].toInt() and 255) shl 16) or
                ((key[o + 3].toInt() and 255) shl 24)
        }
        val out = ByteArray(data.size)
        var offset = 0
        while (offset + 16 <= data.size) {
            var block = data.copyOfRange(offset, offset + 16)
            block = roundA(block, words, 0, 0)
            block = roundA(block, words, 1, 4)
            for (round in 2..15) block = roundB(block, words, round)
            block = roundA(block, words, 16, 64)
            System.arraycopy(block, 0, out, offset, 16)
            offset += 16
        }
        if (offset < data.size) System.arraycopy(data, offset, out, offset, data.size - offset)
        return out
    }

    private fun calculateHash(text: String): Int {
        var result = 0
        for (ch in text) {
            val index = ch.code
            require(index < hashLut.size) { "NG hash name contains a non-ASCII character" }
            val temp = 1025 * ((hashLut[index].toInt() and 255) + result)
            result = (temp ushr 6) xor temp
        }
        val nine = 9 * result
        return 32769 * ((nine ushr 11) xor nine)
    }

    private fun roundA(data: ByteArray, words: IntArray, round: Int, keyOffset: Int): ByteArray {
        val result = ByteArray(16)
        val roundTables = tables[round]
        for (group in 0..3) {
            val base = group * 4
            val value = roundTables[base][data[base].toInt() and 255] xor
                roundTables[base + 1][data[base + 1].toInt() and 255] xor
                roundTables[base + 2][data[base + 2].toInt() and 255] xor
                roundTables[base + 3][data[base + 3].toInt() and 255] xor
                words[keyOffset + group]
            putLe32(result, group * 4, value)
        }
        return result
    }

    private fun roundB(data: ByteArray, words: IntArray, round: Int): ByteArray {
        val keyOffset = round * 4
        val result = ByteArray(16)
        val map = arrayOf(
            intArrayOf(0, 7, 10, 13), intArrayOf(1, 4, 11, 14),
            intArrayOf(2, 5, 8, 15), intArrayOf(3, 6, 9, 12)
        )
        for (group in 0..3) {
            var value = words[keyOffset + group]
            for (j in 0..3) {
                val index = map[group][j]
                value = value xor tables[round][index][data[index].toInt() and 255]
            }
            putLe32(result, group * 4, value)
        }
        return result
    }

    private fun putLe32(out: ByteArray, offset: Int, value: Int) {
        out[offset] = value.toByte()
        out[offset + 1] = (value ushr 8).toByte()
        out[offset + 2] = (value ushr 16).toByte()
        out[offset + 3] = (value ushr 24).toByte()
    }

    fun save(file: File) {
        DataOutputStream(file.outputStream().buffered()).use { out ->
            out.writeInt(0x4E474D31)
            out.writeInt(keys.size)
            keys.forEach(out::write)
            out.writeInt(tables.size)
            tables.forEach { round -> round.forEach { table -> table.forEach(out::writeInt) } }
            out.writeInt(hashLut.size)
            out.write(hashLut)
        }
    }

    companion object {
        fun load(file: File): GtaNgMaterial {
            DataInputStream(file.inputStream().buffered()).use { input ->
                require(input.readInt() == 0x4E474D31) { "Invalid local NG key bundle" }
                val keyCount = input.readInt()
                require(keyCount == 101) { "Invalid NG key count" }
                val keys = Array(keyCount) { ByteArray(272).also(input::readFully) }
                val roundCount = input.readInt()
                require(roundCount == 17) { "Invalid NG table round count" }
                val tables = Array(roundCount) {
                    Array(16) {
                        IntArray(256) { input.readInt() }
                    }
                }
                val lutLength = input.readInt()
                require(lutLength == 256) { "Invalid NG hash LUT size" }
                val lut = ByteArray(lutLength).also(input::readFully)
                require(input.read() == -1) { "Unexpected data at end of NG key bundle" }
                return GtaNgMaterial(keys, tables, lut)
            }
        }

        fun extractFromExe(executable: File, progress: (String, Int) -> Unit = { _, _ -> }): GtaNgMaterial {
            require(executable.isFile && executable.length() > 1024L) { "Select a valid PC GTA5.exe" }
            val keyBytes = GtaNgKeyHashes.keyHashes.map { hex(it) }
            val tableBytes = GtaNgKeyHashes.decryptTableHashes.map { hex(it) }
            val lutBytes = hex(GtaNgKeyHashes.lutHash)
            val keys = searchHashes(executable, 272, keyBytes) { p -> progress("Extracting NG keys", p / 3) }
            val tablesRaw = searchHashes(executable, 1024, tableBytes) { p -> progress("Extracting NG tables", 33 + p / 3) }
            val lutRaw = searchHashes(executable, 256, listOf(lutBytes)) { p -> progress("Extracting NG hash table", 66 + p / 3) }
            val keyValues = keys.mapIndexed { i, value -> value ?: error("NG key fingerprint $i was not found; this GTA5.exe build is not supported.") }.toTypedArray()
            val tableValues = tablesRaw.mapIndexed { i, value -> value ?: error("NG table fingerprint $i was not found; this GTA5.exe build is not supported.") }
            val tables = Array(17) { round ->
                Array(16) { table ->
                    val bytes = tableValues[round * 16 + table]
                    IntArray(256) { i ->
                        val o = i * 4
                        (bytes[o].toInt() and 255) or
                            ((bytes[o + 1].toInt() and 255) shl 8) or
                            ((bytes[o + 2].toInt() and 255) shl 16) or
                            ((bytes[o + 3].toInt() and 255) shl 24)
                    }
                }
            }
            val lut = lutRaw.single() ?: error("NG hash lookup table fingerprint was not found; this GTA5.exe build is not supported.")
            progress("NG key material ready", 100)
            return GtaNgMaterial(keyValues, tables, lut)
        }

        private fun searchHashes(
            executable: File,
            valueLength: Int,
            hashes: List<ByteArray>,
            progress: (Int) -> Unit
        ): Array<ByteArray?> {
            val found = arrayOfNulls<ByteArray>(hashes.size)
            val byPrefix = HashMap<Long, MutableList<Int>>()
            hashes.forEachIndexed { index, hash ->
                require(hash.size == 20)
                byPrefix.getOrPut(prefix(hash)) { mutableListOf() }.add(index)
            }
            val digest = MessageDigest.getInstance("SHA-1")
            val digestBytes = ByteArray(20)
            val blockSize = 1024 * 1024
            val buffer = ByteArray(blockSize + valueLength)
            val total = executable.length()
            RandomAccessFile(executable, "r").use { input ->
                var base = 0L
                while (base < total && found.any { it == null }) {
                    val readLength = minOf(buffer.size.toLong(), total - base).toInt()
                    input.seek(base)
                    input.readFully(buffer, 0, readLength)
                    val scanLimit = minOf(blockSize.toLong(), total - base).toInt()
                    var offset = 0
                    while (offset < scanLimit && offset + valueLength <= readLength) {
                        digest.update(buffer, offset, valueLength)
                        digest.digest(digestBytes, 0, digestBytes.size)
                        val candidates = byPrefix[prefix(digestBytes)]
                        if (candidates != null) {
                            for (index in candidates) {
                                if (hashes[index].contentEquals(digestBytes)) {
                                    found[index] = buffer.copyOfRange(offset, offset + valueLength)
                                }
                            }
                        }
                        offset += 8
                    }
                    base += blockSize.toLong()
                    progress(((minOf(base, total) * 100L) / total).toInt())
                }
            }
            return found
        }

        private fun prefix(bytes: ByteArray): Long {
            var value = 0L
            for (i in 0 until 8) value = (value shl 8) or (bytes[i].toLong() and 255L)
            return value
        }

        private fun hex(value: String): ByteArray =
            value.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    }
}

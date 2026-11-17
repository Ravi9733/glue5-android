package com.ravi9733.rpfexplorer

import java.io.File
import java.io.InputStream
import java.io.OutputStream
import java.io.RandomAccessFile
import java.security.MessageDigest
import javax.crypto.Cipher
import javax.crypto.spec.SecretKeySpec

/**
 * AES key discovery follows the public CodeWalker GTA5Keys approach:
 * scan a user-selected copy of gta5.exe for the 32-byte candidate whose SHA-1
 * matches the published key fingerprint. The key itself is never bundled.
 */
internal object RpfCrypto {
    private val aesKeySha1 = hex("A0796128A775720AC204D9819F68C172E3952C6D")

    fun extractAesKey(executable: File, progress: (Long, Long) -> Unit = { _, _ -> }): ByteArray {
        require(executable.isFile && executable.length() >= 32L) { "Select a valid GTA5.exe file" }
        val total = executable.length()
        val block = 1024 * 1024
        val overlap = 31
        val digest = MessageDigest.getInstance("SHA-1")
        RandomAccessFile(executable, "r").use { input ->
            var base = 0L
            val buffer = ByteArray(block + overlap)
            while (base < total - 31L) {
                val readLength = minOf(buffer.size.toLong(), total - base).toInt()
                input.seek(base)
                input.readFully(buffer, 0, readLength)
                var offset = 0
                val lastStart = minOf(block - 1, readLength - 32)
                while (offset <= lastStart) {
                    digest.reset()
                    digest.update(buffer, offset, 32)
                    if (MessageDigest.isEqual(digest.digest(), aesKeySha1)) {
                        return buffer.copyOfRange(offset, offset + 32)
                    }
                    offset += 8
                }
                base += block.toLong()
                progress(minOf(base, total), total)
            }
        }
        error("GTA V AES key fingerprint was not found. This executable may be a different game build or platform.")
    }

    fun decryptAes(data: ByteArray, key: ByteArray): ByteArray {
        require(key.size == 32) { "GTA V AES key must be exactly 32 bytes" }
        val fullLength = data.size - (data.size % 16)
        val result = data.clone()
        if (fullLength > 0) {
            val cipher = Cipher.getInstance("AES/ECB/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, SecretKeySpec(key, "AES"))
            val decrypted = cipher.doFinal(data, 0, fullLength)
            System.arraycopy(decrypted, 0, result, 0, fullLength)
        }
        return result
    }

    fun decryptAes(input: InputStream, size: Long, output: OutputStream, key: ByteArray) {
        require(key.size == 32) { "GTA V AES key must be exactly 32 bytes" }
        require(size >= 0) { "Invalid encrypted payload size" }
        val cipher = Cipher.getInstance("AES/ECB/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, SecretKeySpec(key, "AES"))
        val buffer = ByteArray(64 * 1024 + 16)
        var carry = 0
        var remaining = size
        while (remaining > 0) {
            val request = minOf((buffer.size - carry).toLong(), remaining).toInt()
            val n = input.read(buffer, carry, request)
            if (n < 0) error("Unexpected end of encrypted payload")
            if (n == 0) continue
            remaining -= n
            val total = carry + n
            val processLength = if (remaining == 0L) total - (total % 16) else total - (total % 16)
            if (processLength > 0) {
                cipher.update(buffer, 0, processLength)?.let { output.write(it) }
            }
            carry = total - processLength
            if (carry > 0) System.arraycopy(buffer, processLength, buffer, 0, carry)
        }
        if (carry > 0) output.write(buffer, 0, carry) // AES leaves the final partial block unchanged.
        cipher.doFinal()?.let { output.write(it) }
    }

    private fun hex(value: String): ByteArray =
        value.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
}

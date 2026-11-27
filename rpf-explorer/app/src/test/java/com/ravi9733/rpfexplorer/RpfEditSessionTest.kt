package com.ravi9733.rpfexplorer

import java.io.ByteArrayOutputStream
import java.io.ByteArrayInputStream
import java.io.File
import java.io.FileOutputStream
import java.io.RandomAccessFile
import java.util.zip.Deflater
import java.util.zip.DeflaterOutputStream
import java.security.MessageDigest
import javax.crypto.Cipher
import javax.crypto.spec.SecretKeySpec
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class RpfEditSessionTest {
    private fun u32(out: RandomAccessFile, value: Long) {
        out.write((value and 255).toInt())
        out.write(((value ushr 8) and 255).toInt())
        out.write(((value ushr 16) and 255).toInt())
        out.write(((value ushr 24) and 255).toInt())
    }

    private fun fixture(dir: File): File {
        val f = File(dir, "fixture.rpf")
        val names = "root\u0000hello.txt\u0000".toByteArray(Charsets.UTF_8)
        RandomAccessFile(f, "rw").use { r ->
            r.setLength(0)
            u32(r, 0x52504637L); u32(r, 2); u32(r, names.size.toLong()); u32(r, 0)
            // Root directory: one child at TOC index 1.
            u32(r, 0); u32(r, 0x7FFFFF00L); u32(r, 1); u32(r, 1)
            // Uncompressed binary "hello", beginning at 512-byte block 1.
            r.write(5); r.write(0); r.write(0); r.write(0); r.write(0)
            r.write(1); r.write(0); r.write(0)
            u32(r, 5); u32(r, 0)
            r.write(names)
            r.seek(512); r.write("hello".toByteArray())
        }
        return f
    }

    private fun sha(file: File): String =
        MessageDigest.getInstance("SHA-256").digest(file.readBytes()).joinToString("") { "%02x".format(it) }

    @Test fun addReplaceSaveReopenKeepsOriginalUntouched() {
        val dir = createTempDir(prefix = "rpf-test-")
        try {
            val source = fixture(dir)
            val before = sha(source)
            val archive = RpfArchive.open(source)
            val work = File(dir, "work").apply { mkdirs() }
            val editor = RpfEditSession(archive, work)
            val replacement = File(dir, "replacement.bin").apply { writeText("world!!") }
            val addition = File(dir, "addition.bin").apply { writeText("extra") }
            editor.replace(1, replacement)
            editor.add(0, addition, "addon.bin")
            val output = File(dir, "edited.rpf")
            editor.saveAs(output)

            val reopened = RpfArchive.open(output)
            assertEquals(3, reopened.entries.size)
            val originalEntry = reopened.entries.first { it.name == "hello.txt" }
            val addedEntry = reopened.entries.first { it.name == "addon.bin" }
            val replacedBytes = ByteArrayOutputStream().also { reopened.extract(originalEntry, it) }.toByteArray()
            val addedBytes = ByteArrayOutputStream().also { reopened.extract(addedEntry, it) }.toByteArray()
            assertEquals("world!!", String(replacedBytes))
            assertEquals("extra", String(addedBytes))
            assertEquals(before, sha(source))
        } finally {
            dir.deleteRecursively()
        }
    }
 
    private fun resourceFixture(
        dir: File,
        resourceName: String = "sample.ytd",
        encryption: Long = 0x4E45504FL,
        aesKey: ByteArray? = null
    ): File {
        val f = File(dir, "resource.rpf")
        val names = "root\u0000${resourceName}\u0000".toByteArray(Charsets.UTF_8)
        RandomAccessFile(File(dir, "resource-toc.bin"), "rw").use { r ->
            r.setLength(0)
            u32(r, 0); u32(r, 0x7FFFFF00L); u32(r, 1); u32(r, 1)
            r.write(5); r.write(0); r.write(16); r.write(0); r.write(0)
            r.write(1); r.write(0); r.write(0x80)
            u32(r, 0x12345678L); u32(r, 0x23456789L)
        }
        val tocBytes = File(dir, "resource-toc.bin").readBytes()
        val namesBytes = if (encryption == 0x0FFFFFF9L) {
            encryptAes(names, aesKey ?: error("AES key required"))
        } else names
        val entryBytes = if (encryption == 0x0FFFFFF9L) {
            encryptAes(tocBytes, aesKey ?: error("AES key required"))
        } else tocBytes
        RandomAccessFile(f, "rw").use { r ->
            r.setLength(0)
            u32(r, 0x52504637L); u32(r, 2); u32(r, names.size.toLong()); u32(r, encryption)
            r.write(entryBytes); r.write(namesBytes)
            r.seek(512)
            r.write(byteArrayOf(0x52,0x53,0x43,0x37, 1,0,0,0, 0x78,0x56,0x34,0x12, 0x89.toByte(),0x67,0x45,0x23))
        }
        return f
    }

    private fun encryptedAesYscFixture(dir: File, key: ByteArray, script: ByteArray): File {
        val compressedBuffer = ByteArrayOutputStream()
        DeflaterOutputStream(compressedBuffer, Deflater(Deflater.DEFAULT_COMPRESSION, true)).use { it.write(script) }
        val encryptedPayload = encryptAes(compressedBuffer.toByteArray(), key)
        val resourceSize = 16 + encryptedPayload.size
        require(resourceSize < 0xFFFFFF)
        val f = File(dir, "encrypted-script.rpf")
        val names = "root\u0000main.ysc\u0000".toByteArray(Charsets.UTF_8)
        val toc = File(dir, "ysc-toc.bin")
        RandomAccessFile(toc, "rw").use { out ->
            out.setLength(0)
            u32(out, 0); u32(out, 0x7FFFFF00L); u32(out, 1); u32(out, 1)
            out.write(5); out.write(0)
            out.write(resourceSize and 255); out.write((resourceSize ushr 8) and 255); out.write((resourceSize ushr 16) and 255)
            out.write(1); out.write(0); out.write(0x80)
            u32(out, 0x12345678L); u32(out, 0x23456789L)
        }
        RandomAccessFile(f, "rw").use { out ->
            out.setLength(0)
            u32(out, 0x52504637L); u32(out, 2); u32(out, names.size.toLong()); u32(out, 0x0FFFFFF9L)
            out.write(encryptAes(toc.readBytes(), key))
            out.write(encryptAes(names, key))
            out.seek(512)
            out.write(byteArrayOf(0x52,0x53,0x43,0x37, 1,0,0,0, 0x78,0x56,0x34,0x12, 0x89.toByte(),0x67,0x45,0x23))
            out.write(encryptedPayload)
        }
        return f
    }

    @Test fun encryptedAesYscCanBeExportedAsDecryptedStandaloneScript() {
        val dir = createTempDir(prefix = "rpf-ysc-export-test-")
        try {
            val key = ByteArray(32) { (it * 17 + 3).toByte() }
            val expected = ByteArray(8193) { (it * 31).toByte() }
            val source = encryptedAesYscFixture(dir, key, expected)
            val archive = RpfArchive.open(source, key)
            val entry = archive.entries.single { it.name == "main.ysc" }
            assertTrue(archive.canDecryptYsc(entry))
            val actual = ByteArrayOutputStream().also { archive.extractEncryptedYsc(entry, it) }.toByteArray()
            assertTrue(expected.contentEquals(actual))
        } finally {
            dir.deleteRecursively()
        }
    }

    private fun encryptAes(bytes: ByteArray, key: ByteArray): ByteArray {
        val full = bytes.size - bytes.size % 16
        val result = bytes.clone()
        if (full > 0) {
            val cipher = Cipher.getInstance("AES/ECB/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, SecretKeySpec(key, "AES"))
            val encrypted = cipher.doFinal(bytes, 0, full)
            System.arraycopy(encrypted, 0, result, 0, full)
        }
        return result
    }

    private fun aesFixture(dir: File, key: ByteArray): Pair<File, ByteArray> {
        val f = File(dir, "aes-fixture.rpf")
        val names = "root\u0000hello.txt\u0000\u0000".toByteArray(Charsets.UTF_8)
        val payload = "0123456789ABCDEF".toByteArray(Charsets.UTF_8)
        val toc = ByteArrayOutputStream()
        val tocFile = File(dir, "toc-plain.bin")
        RandomAccessFile(tocFile, "rw").use { r ->
            r.setLength(0)
            u32(r, 0); u32(r, 0x7FFFFF00L); u32(r, 1); u32(r, 1)
            r.write(5); r.write(0); r.write(0); r.write(0); r.write(0)
            r.write(1); r.write(0); r.write(0)
            u32(r, payload.size.toLong()); u32(r, 1)
        }
        val encryptedToc = encryptAes(tocFile.readBytes(), key)
        val encryptedNames = encryptAes(names, key)
        val encryptedPayload = encryptAes(payload, key)
        RandomAccessFile(f, "rw").use { r ->
            r.setLength(0)
            u32(r, 0x52504637L); u32(r, 2); u32(r, names.size.toLong()); u32(r, 0x0FFFFFF9L)
            r.write(encryptedToc); r.write(encryptedNames)
            r.seek(512); r.write(encryptedPayload)
        }
        return f to payload
    }

    @Test fun aesTocAndBinaryPayloadDecryptThenRepackAsOpen() {
        val dir = createTempDir(prefix = "rpf-aes-test-")
        try {
            val key = ByteArray(32) { (it * 7 + 3).toByte() }
            val (source, expected) = aesFixture(dir, key)
            val archive = RpfArchive.open(source, key)
            assertEquals("AES", archive.encryptionLabel)
            val entry = archive.entries.single { it.name == "hello.txt" }
            val extracted = ByteArrayOutputStream().also { archive.extract(entry, it) }.toByteArray()
            assertTrue(expected.contentEquals(extracted))
            val output = File(dir, "decrypted-open.rpf")
            RpfEditSession(archive, File(dir, "work").apply { mkdirs() }).saveAs(output)
            val reopened = RpfArchive.open(output)
            assertEquals("NONE", reopened.encryptionLabel)
            val outputEntry = reopened.entries.single { it.name == "hello.txt" }
            val outputBytes = ByteArrayOutputStream().also { reopened.extract(outputEntry, it) }.toByteArray()
            assertTrue(expected.contentEquals(outputBytes))
        } finally {
            dir.deleteRecursively()
        }
    }


    @Test fun streamingAesDecryptMatchesBlockDecryptAcrossBufferBoundaries() {
        val key = ByteArray(32) { (it * 11 + 5).toByte() }
        val plain = ByteArray(150_013) { (it * 13).toByte() }
        val encrypted = encryptAes(plain, key)
        val streamed = ByteArrayOutputStream()
        RpfCrypto.decryptAes(ByteArrayInputStream(encrypted), encrypted.size.toLong(), streamed, key)
        assertTrue(plain.contentEquals(streamed.toByteArray()))
    }



    private fun identityNgMaterial(): GtaNgMaterial {
        val keys = Array(101) { ByteArray(272) }
        val tables = Array(17) { Array(16) { IntArray(256) } }
        for (round in listOf(0, 1, 16)) {
            for (group in 0..3) {
                for (slot in 0..3) {
                    val tableIndex = group * 4 + slot
                    for (value in 0..255) tables[round][tableIndex][value] = value shl (slot * 8)
                }
            }
        }
        val map = arrayOf(
            intArrayOf(0, 7, 10, 13), intArrayOf(1, 4, 11, 14),
            intArrayOf(2, 5, 8, 15), intArrayOf(3, 6, 9, 12)
        )
        for (round in 2..15) {
            for (group in 0..3) {
                for (slot in 0..3) {
                    val tableIndex = map[group][slot]
                    for (value in 0..255) tables[round][tableIndex][value] = value shl (slot * 8)
                }
            }
        }
        return GtaNgMaterial(keys, tables, ByteArray(256) { it.toByte() })
    }

    private fun encryptIdentityNg(plain: ByteArray): ByteArray {
        val result = plain.clone()
        val map = arrayOf(
            intArrayOf(0, 7, 10, 13), intArrayOf(1, 4, 11, 14),
            intArrayOf(2, 5, 8, 15), intArrayOf(3, 6, 9, 12)
        )
        var offset = 0
        while (offset + 16 <= plain.size) {
            var block = plain.copyOfRange(offset, offset + 16)
            for (round in 15 downTo 2) {
                val inverse = ByteArray(16)
                for (group in 0..3) for (slot in 0..3) {
                    inverse[map[group][slot]] = block[group * 4 + slot]
                }
                block = inverse
            }
            System.arraycopy(block, 0, result, offset, 16)
            offset += 16
        }
        return result
    }

    @Test fun ngArchiveTocAndEncryptedBinaryRoundTripAsNone() {
        val dir = createTempDir(prefix = "rpf-ng-archive-test-")
        try {
            val material = identityNgMaterial()
            val names = "root\u0000hello.txt\u0000".toByteArray(Charsets.UTF_8)
            val plainPayload = "0123456789ABCDEF".toByteArray(Charsets.UTF_8)
            val toc = ByteArrayOutputStream()
            val tocFile = File(dir, "toc.bin")
            RandomAccessFile(tocFile, "rw").use { r ->
                r.setLength(0)
                u32(r, 0); u32(r, 0x7FFFFF00L); u32(r, 1); u32(r, 1)
                r.write(5); r.write(0); r.write(0); r.write(0); r.write(0)
                r.write(1); r.write(0); r.write(0)
                u32(r, plainPayload.size.toLong()); u32(r, 1)
            }
            val encryptedToc = encryptIdentityNg(tocFile.readBytes())
            val encryptedNames = encryptIdentityNg(names)
            val encryptedPayload = encryptIdentityNg(plainPayload)
            val source = File(dir, "ng.rpf")
            RandomAccessFile(source, "rw").use { r ->
                r.setLength(0)
                u32(r, 0x52504637L); u32(r, 2); u32(r, names.size.toLong()); u32(r, 0x0FEFFFFFL)
                r.write(encryptedToc); r.write(encryptedNames)
                r.seek(512); r.write(encryptedPayload)
            }
            val archive = RpfArchive.open(source, ngMaterial = material, archiveName = "ng.rpf")
            assertEquals("NG", archive.encryptionLabel)
            val entry = archive.entries.single { it.name == "hello.txt" }
            val extracted = ByteArrayOutputStream().also { archive.extract(entry, it) }.toByteArray()
            assertTrue(plainPayload.contentEquals(extracted))

            val output = File(dir, "ng-open.rpf")
            RpfEditSession(archive, File(dir, "work").apply { mkdirs() }).saveAs(output)
            val reopened = RpfArchive.open(output)
            assertEquals("NONE", reopened.encryptionLabel)
            val outputEntry = reopened.entries.single { it.name == "hello.txt" }
            assertTrue(plainPayload.contentEquals(ByteArrayOutputStream().also { reopened.extract(outputEntry, it) }.toByteArray()))
        } finally {
            dir.deleteRecursively()
        }
    }

    @Test fun ngDecryptStreamingAndPersistedMaterialMatchArrayPath() {
        val keys = Array(101) { key -> ByteArray(272) { i -> (key * 17 + i * 3).toByte() } }
        val tables = Array(17) { round ->
            Array(16) { table ->
                IntArray(256) { value -> (round * 0x1020304) xor (table * 0x10001) xor (value * 0x01010101) }
            }
        }
        val lut = ByteArray(256) { it.toByte() }
        val material = GtaNgMaterial(keys, tables, lut)
        val encrypted = ByteArray(40_019) { (it * 29 + 7).toByte() }
        val expected = material.decrypt(encrypted, "update.rpf", 123_456L)
        val expectedHash = MessageDigest.getInstance("SHA-256").digest(expected).joinToString("") { "%02x".format(it) }
        assertEquals("50a455e2735986c62e90eacbf88638435b10f7048817f7c63c1d5f18c5a8b761", expectedHash)
        val streamed = ByteArrayOutputStream()
        material.decrypt(ByteArrayInputStream(encrypted), encrypted.size.toLong(), streamed, "update.rpf", 123_456L)
        assertTrue(expected.contentEquals(streamed.toByteArray()))

        val dir = createTempDir(prefix = "rpf-ng-material-test-")
        try {
            val saved = File(dir, "ng-material.bin")
            material.save(saved)
            val loaded = GtaNgMaterial.load(saved)
            assertTrue(expected.contentEquals(loaded.decrypt(encrypted, "update.rpf", 123_456L)))
        } finally {
            dir.deleteRecursively()
        }
    }

    @Test fun resourcePayloadAndPageFlagsSurviveRepack() {
        val dir = createTempDir(prefix = "rpf-resource-test-")
        try {
            val source = resourceFixture(dir)
            val archive = RpfArchive.open(source)
            val resource = archive.entries.single { it.isResource }
            val replacement = File(dir, "replacement.ytd")
            val replacementBytes = byteArrayOf(
                0x52,0x53,0x43,0x37, 1,0,0,0,
                0x11,0x11,0x11,0x11, 0x22,0x22,0x22,0x22
            )
            replacement.writeBytes(replacementBytes)
            val addition = File(dir, "addition.ytd")
            addition.writeBytes(byteArrayOf(
                0x52,0x53,0x43,0x37, 1,0,0,0,
                0x33,0x33,0x33,0x33, 0x44,0x44,0x44,0x44
            ))
            val editor = RpfEditSession(archive, File(dir, "work").apply { mkdirs() })
            editor.replace(resource.index, replacement)
            editor.add(0, addition, "added.ytd")
            val output = File(dir, "resource-copy.rpf")
            editor.saveAs(output)
            val reopened = RpfArchive.open(output)
            val copied = reopened.entries.single { it.name == "sample.ytd" }
            val added = reopened.entries.single { it.name == "added.ytd" }
            assertEquals(0x11111111L, copied.systemFlags)
            assertEquals(0x22222222L, copied.graphicsFlags)
            assertEquals(0x33333333L, added.systemFlags)
            assertEquals(0x44444444L, added.graphicsFlags)
            val after = ByteArrayOutputStream().also { reopened.copyRaw(copied, it) }.toByteArray()
            assertTrue(replacementBytes.contentEquals(after))
        } finally {
            dir.deleteRecursively()
        }
    }



    @Test fun unencryptedRsc7ResourceCanBeEditedInsideAesArchive() {
        val dir = createTempDir(prefix = "rpf-aes-resource-test-")
        try {
            val key = ByteArray(32) { (it * 7 + 9).toByte() }
            val source = resourceFixture(dir, "sample.ytd", 0x0FFFFFF9L, key)
            val archive = RpfArchive.open(source, key)
            assertEquals("AES", archive.encryptionLabel)
            val resource = archive.entries.single { it.isResource }
            val replacement = File(dir, "replacement.ytd")
            val bytes = byteArrayOf(0x52,0x53,0x43,0x37, 2,0,0,0, 0x55,0x55,0x55,0x55, 0x66,0x66,0x66,0x66)
            replacement.writeBytes(bytes)
            val editor = RpfEditSession(archive, File(dir, "work").apply { mkdirs() })
            editor.replace(resource.index, replacement)
            val output = File(dir, "aes-resource-open.rpf")
            editor.saveAs(output)
            val reopened = RpfArchive.open(output)
            assertEquals("NONE", reopened.encryptionLabel)
            val result = reopened.entries.single { it.name == "sample.ytd" }
            assertEquals(0x55555555L, result.systemFlags)
            assertEquals(0x66666666L, result.graphicsFlags)
            assertTrue(bytes.contentEquals(ByteArrayOutputStream().also { reopened.copyRaw(result, it) }.toByteArray()))
        } finally {
            dir.deleteRecursively()
        }
    }

    @Test fun encryptedYscResourceIsDetectedAndBlocksUnsafeRepack() {
        val dir = createTempDir(prefix = "rpf-ysc-test-")
        try {
            val source = resourceFixture(dir, "script.ysc")
            val archive = RpfArchive.open(source)
            val script = archive.entries.single { it.isResource }
            assertTrue(script.encrypted)
            val editor = RpfEditSession(archive, File(dir, "work").apply { mkdirs() })
            val output = File(dir, "unsafe.rpf")
            assertTrue(runCatching { editor.saveAs(output) }.isFailure)
            editor.remove(script.index)
            editor.saveAs(output)
            val reopened = RpfArchive.open(output)
            assertTrue(reopened.entries.none { it.name.endsWith(".ysc", true) })
        } finally {
            dir.deleteRecursively()
        }
    }

    @Test fun ngStyleYscResourceIsPreservedByteForByteInOpenArchiveRepack() {
        val dir = createTempDir(prefix = "rpf-ysc-preserve-test-")
        try {
            val source = resourceFixture(dir, "main.ysc")
            val archive = RpfArchive.open(source)
            val script = archive.entries.single { it.isResource }
            assertTrue(script.encrypted)
            val before = ByteArrayOutputStream().also { archive.copyRaw(script, it) }.toByteArray()
            val output = File(dir, "preserved-script.rpf")
            RpfEditSession(archive, File(dir, "work").apply { mkdirs() }).saveAs(output)
            val reopened = RpfArchive.open(output)
            assertEquals("OPEN", reopened.encryptionLabel)
            val copied = reopened.entries.single { it.name == "main.ysc" }
            assertTrue(copied.encrypted)
            val after = ByteArrayOutputStream().also { reopened.copyRaw(copied, it) }.toByteArray()
            assertTrue(before.contentEquals(after))
        } finally {
            dir.deleteRecursively()
        }
    }

    @Test fun compressedBinaryIsInflatedAndRepackedUncompressed() {
        val dir = createTempDir(prefix = "rpf-deflate-test-")
        try {
            val plain = "compressed GTA V archive entry ".repeat(40).toByteArray()
            val compressedOut = ByteArrayOutputStream()
            DeflaterOutputStream(compressedOut, Deflater(Deflater.DEFAULT_COMPRESSION, true)).use { it.write(plain) }
            val compressed = compressedOut.toByteArray()
            val names = "root\u0000stream.dat\u0000".toByteArray(Charsets.UTF_8)
            val source = File(dir, "compressed.rpf")
            RandomAccessFile(source, "rw").use { r ->
                r.setLength(0)
                u32(r, 0x52504637L); u32(r, 2); u32(r, names.size.toLong()); u32(r, 0)
                u32(r, 0); u32(r, 0x7FFFFF00L); u32(r, 1); u32(r, 1)
                r.write(5); r.write(0)
                r.write(compressed.size and 255); r.write((compressed.size ushr 8) and 255); r.write((compressed.size ushr 16) and 255)
                r.write(1); r.write(0); r.write(0)
                u32(r, plain.size.toLong()); u32(r, 0)
                r.write(names); r.seek(512); r.write(compressed)
            }
            val archive = RpfArchive.open(source)
            val entry = archive.entries.single { it.name == "stream.dat" }
            assertTrue(plain.contentEquals(ByteArrayOutputStream().also { archive.extract(entry, it) }.toByteArray()))
            val output = File(dir, "repacked.rpf")
            RpfEditSession(archive, File(dir, "work").apply { mkdirs() }).saveAs(output)
            val reopened = RpfArchive.open(output)
            val newEntry = reopened.entries.single { it.name == "stream.dat" }
            assertEquals(0L, newEntry.storedSize)
            assertTrue(plain.contentEquals(ByteArrayOutputStream().also { reopened.extract(newEntry, it) }.toByteArray()))
        } finally {
            dir.deleteRecursively()
        }
    }

    @Test fun nestedDirectoryRangesRemainValidAfterBreadthFirstRebuild() {
        val dir = createTempDir(prefix = "rpf-nested-dir-test-")
        try {
            val f = File(dir, "nested-dir.rpf")
            val names = "root\u0000sub\u0000top.txt\u0000inside.txt\u0000".toByteArray(Charsets.UTF_8)
            RandomAccessFile(f, "rw").use { r ->
                r.setLength(0)
                u32(r, 0x52504637L); u32(r, 4); u32(r, names.size.toLong()); u32(r, 0)
                u32(r, 0); u32(r, 0x7FFFFF00L); u32(r, 1); u32(r, 2)
                u32(r, 5); u32(r, 0x7FFFFF00L); u32(r, 3); u32(r, 1)
                r.write(9); r.write(0); r.write(0); r.write(0); r.write(0)
                r.write(1); r.write(0); r.write(0); u32(r, 3); u32(r, 0)
                r.write(17); r.write(0); r.write(0); r.write(0); r.write(0)
                r.write(2); r.write(0); r.write(0); u32(r, 5); u32(r, 0)
                r.write(names)
                r.seek(512); r.write("top".toByteArray())
                r.seek(1024); r.write("inner".toByteArray())
            }
            val archive = RpfArchive.open(f)
            val output = File(dir, "nested-rebuilt.rpf")
            RpfEditSession(archive, File(dir, "work").apply { mkdirs() }).saveAs(output)
            val reopened = RpfArchive.open(output)
            val rootChildren = reopened.children(0).map { it.name }.toSet()
            assertTrue(rootChildren.contains("sub"))
            assertTrue(rootChildren.contains("top.txt"))
            val sub = reopened.entries.single { it.name == "sub" }
            assertEquals(listOf("inside.txt"), reopened.children(sub.index).map { it.name })
        } finally {
            dir.deleteRecursively()
        }
    }

    @Test fun removeEntryIsReflectedInSavedArchive() {
        val dir = createTempDir(prefix = "rpf-remove-test-")
        try {
            val source = fixture(dir)
            val archive = RpfArchive.open(source)
            val editor = RpfEditSession(archive, File(dir, "work").apply { mkdirs() })
            editor.remove(1)
            val output = File(dir, "removed.rpf")
            editor.saveAs(output)
            val reopened = RpfArchive.open(output)
            assertEquals(1, reopened.entries.size)
            assertTrue(reopened.entries.single().isDirectory)
            assertFalse(editor.isRemoved(0))
        } finally {
            dir.deleteRecursively()
        }
    }
}

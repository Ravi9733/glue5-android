package com.ravi9733.rpfexplorer

import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileOutputStream
import java.io.RandomAccessFile
import java.security.MessageDigest
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

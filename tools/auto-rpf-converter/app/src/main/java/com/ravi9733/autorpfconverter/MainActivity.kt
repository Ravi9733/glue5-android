package com.ravi9733.autorpconverter

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.view.Gravity
import android.view.ViewGroup
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import androidx.documentfile.provider.DocumentFile
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.Executors

class MainActivity : Activity() {
    private val worker = Executors.newSingleThreadExecutor()
    private lateinit var output: TextView
    private var latestReport: String = "No scan has been run yet."

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(24, 28, 24, 24)
            setBackgroundColor(0xFF101510.toInt())
        }

        val title = TextView(this).apply {
            text = "GTA V AUTO RPF CONVERTER"
            textSize = 21f
            setTextColor(0xFF9BE564.toInt())
            gravity = Gravity.CENTER
        }
        root.addView(title, LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT
        ))

        val subtitle = TextView(this).apply {
            text = "Prototype 0.2 • RPF7 header and encryption inspection"
            textSize = 13f
            setTextColor(0xFFB9C6B5.toInt())
            gravity = Gravity.CENTER
            setPadding(0, 10, 0, 22)
        }
        root.addView(subtitle)

        val choose = Button(this).apply {
            text = "SELECT GTA V FOLDER"
            setOnClickListener {
                val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE)
                intent.addFlags(
                    Intent.FLAG_GRANT_READ_URI_PERMISSION or
                    Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION
                )
                startActivityForResult(intent, REQUEST_FOLDER)
            }
        }
        root.addView(choose, LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT
        ))

        val share = Button(this).apply {
            text = "SHARE SCAN REPORT"
            setOnClickListener {
                val send = Intent(Intent.ACTION_SEND).apply {
                    type = "text/plain"
                    putExtra(Intent.EXTRA_SUBJECT, "GTA V RPF inventory")
                    putExtra(Intent.EXTRA_TEXT, latestReport)
                }
                startActivity(Intent.createChooser(send, "Share report"))
            }
        }
        root.addView(share)

        output = TextView(this).apply {
            text = "Select your GTA V folder. The app checks RPF7 headers, archive entry counts and encryption markers.\n\nThis prototype is read-only and does not convert files."
            textSize = 14f
            setTextColor(0xFFE4EAE1.toInt())
            setPadding(4, 18, 4, 18)
        }
        val scroll = ScrollView(this).apply { addView(output) }
        root.addView(scroll, LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f
        ))
        setContentView(root)
    }

    @Deprecated("Deprecated in Android 13, kept for broad device compatibility")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != REQUEST_FOLDER || resultCode != Activity.RESULT_OK) return
        val uri = data?.data ?: return
        val flags = data.flags and (
            Intent.FLAG_GRANT_READ_URI_PERMISSION or
            Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        )
        contentResolver.takePersistableUriPermission(uri, flags)
        output.text = "Scanning folder… please keep the app open."
        worker.execute {
            val report = scanTree(uri)
            runOnUiThread {
                latestReport = report
                output.text = report
            }
        }
    }

    private data class HeaderInfo(
        val description: String,
        val entryCount: Long? = null,
        val encryption: String = "Unknown",
        val isGtaV: Boolean = false
    )

    private fun inspectHeader(uri: Uri): HeaderInfo {
        val header = try {
            contentResolver.openInputStream(uri)?.use { input ->
                val bytes = ByteArray(16)
                var offset = 0
                while (offset < bytes.size) {
                    val n = input.read(bytes, offset, bytes.size - offset)
                    if (n <= 0) break
                    offset += n
                }
                if (offset == 16) bytes else bytes.copyOf(offset)
            } ?: ByteArray(0)
        } catch (_: Exception) {
            return HeaderInfo("Unreadable header")
        }

        if (header.size < 16) return HeaderInfo("Header shorter than 16 bytes")
        val magic = header.copyOfRange(0, 4).toString(Charsets.US_ASCII)
        if (magic != "RPF7") return HeaderInfo("Magic=$magic (not GTA V RPF7)")

        val bb = ByteBuffer.wrap(header).order(ByteOrder.LITTLE_ENDIAN)
        val entries = bb.getInt(4).toLong() and 0xFFFFFFFFL
        val encryptionWord = bb.getInt(12)
        val encryption = when (encryptionWord) {
            0x4E45504F -> "OPEN (no archive key needed)"
            0x0FFFFFF9 -> "AES encrypted (key material required)"
            0x0FEFFFFF -> "NG encrypted (key material required)"
            0 -> "None"
            else -> "Unknown marker 0x%08X".format(encryptionWord)
        }
        val namesLength = bb.getInt(8).toLong() and 0x0FFFFFFFL
        val tocEnd = 16L + entries * 16L + namesLength
        val bounds = if (tocEnd <= (Long.MAX_VALUE / 2)) "TOC metadata present" else "Invalid TOC length"
        return HeaderInfo(
            "GTA V RPF7 • $bounds",
            entries,
            encryption,
            true
        )
    }

    private fun scanTree(rootUri: Uri): String {
        val root = DocumentFile.fromTreeUri(this, rootUri)
            ?: return "Could not open selected folder."
        val rows = mutableListOf<String>()
        var totalBytes = 0L
        var gtaVArchives = 0
        var openArchives = 0
        var encryptedArchives = 0
        var unreadable = 0
        val stack = ArrayDeque<DocumentFile>()
        stack.add(root)

        while (stack.isNotEmpty()) {
            val dir = stack.removeLast()
            val children = try { dir.listFiles() } catch (_: Exception) { emptyArray() }
            for (child in children) {
                if (child.isDirectory) {
                    stack.add(child)
                } else if (child.isFile && child.name?.endsWith(".rpf", true) == true) {
                    val size = child.length()
                    totalBytes += size
                    val info = inspectHeader(child.uri)
                    if (info.isGtaV) {
                        gtaVArchives++
                        if (info.encryption.startsWith("OPEN")) openArchives++
                        if (info.encryption.contains("encrypted")) encryptedArchives++
                    } else if (info.description.startsWith("Unreadable")) {
                        unreadable++
                    }
                    rows.add(
                        "${child.name} | ${"%.2f".format(size / 1048576.0)} MiB | " +
                        "${info.description} | entries=${info.entryCount ?: "?"} | " +
                        "encryption=${info.encryption}"
                    )
                }
            }
        }

        rows.sort()
        return buildString {
            appendLine("GTA V RPF INVENTORY — PROTOTYPE 0.2")
            appendLine("RPF files found: ${rows.size}")
            appendLine("GTA V RPF7 archives: $gtaVArchives")
            appendLine("OPEN archives: $openArchives")
            appendLine("AES/NG encrypted archives: $encryptedArchives")
            appendLine("Unreadable headers: $unreadable")
            appendLine("Total RPF size: ${"%.2f".format(totalBytes / 1073741824.0)} GiB")
            appendLine()
            rows.forEach { appendLine(it) }
            appendLine()
            appendLine("Read-only scan. No source files were changed.")
            appendLine("Encrypted RPF contents cannot be inspected without the matching key material.")
            appendLine("RPF7 detection is not proof that the archive's assets are compatible with a specific Android game port.")
            appendLine("This build does not yet extract, transform or convert assets.")
        }
    }

    override fun onDestroy() {
        worker.shutdown()
        super.onDestroy()
    }

    companion object {
        private const val REQUEST_FOLDER = 1001
    }
}

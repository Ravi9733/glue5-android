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
    private var selectedArchiveUri: Uri? = null
    private var selectedArchiveName: String = "selected.rpf"
    private var outputTreeUri: Uri? = null

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
            text = "Prototype 0.3 • OPEN RPF7 TOC and asset inventory"
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
        val chooseArchive = Button(this).apply {
            text = "SELECT ONE .RPF ARCHIVE"
            setOnClickListener {
                val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    type = "*/*"
                    addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
                }
                startActivityForResult(intent, REQUEST_ARCHIVE)
            }
        }
        root.addView(chooseArchive)
        val chooseDestination = Button(this).apply {
            text = "SELECT EXTRACTION OUTPUT FOLDER"
            setOnClickListener {
                val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
                    addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
                }
                startActivityForResult(intent, REQUEST_DESTINATION)
            }
        }
        root.addView(chooseDestination)
        val extract = Button(this).apply {
            text = "EXTRACT SELECTED RPF"
            setOnClickListener { extractSelectedArchive() }
        }
        root.addView(extract)

        root.addView(share)

        output = TextView(this).apply {
            text = "Select your GTA V folder. The app checks RPF7 headers, archive entry counts and encryption markers.\n\nThis prototype reads OPEN RPF7 tables of contents. It does not modify source archives or convert assets."
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
        if (resultCode != Activity.RESULT_OK) return
        val uri = data?.data ?: return
        val flags = data.flags and (Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        try { contentResolver.takePersistableUriPermission(uri, flags) } catch (_: Exception) {}
        when (requestCode) {
            REQUEST_FOLDER -> worker.execute {
                val report = scanTree(uri)
                runOnUiThread { latestReport = report; output.text = report }
            }
            REQUEST_ARCHIVE -> {
                selectedArchiveUri = uri
                selectedArchiveName = DocumentFile.fromSingleUri(this, uri)?.name ?: "selected.rpf"
                output.text = "Selected " + selectedArchiveName + ". Select output folder next."
            }
            REQUEST_DESTINATION -> {
                outputTreeUri = uri
                output.text = "Output folder selected. Tap EXTRACT SELECTED RPF."
            }
        }
    }

    private fun extractSelectedArchive() {
        val sourceUri = selectedArchiveUri
        val destinationUri = outputTreeUri
        if (sourceUri == null || destinationUri == null) { output.text = "Select an RPF and output folder first."; return }
        if (!selectedArchiveName.endsWith(".rpf", true)) { output.text = "Choose a .rpf file."; return }
        output.text = "Extracting. Keep the app open; this can take time."
        worker.execute {
            val result = try {
                val work = java.io.File(cacheDir, "rpf-work")
                work.deleteRecursively()
                check(work.mkdirs()) { "Cannot create temporary folder" }
                val safeName = selectedArchiveName.replace(Regex("[^A-Za-z0-9._-]"), "_")
                val source = java.io.File(work, safeName)
                contentResolver.openInputStream(sourceUri)?.use { input -> source.outputStream().use { out -> input.copyTo(out) } } ?: error("Cannot read archive")
                val extracted = java.io.File(work, "extracted")
                check(extracted.mkdir()) { "Cannot create extraction folder" }
                val report = NativeRpf.extractArchive(source.absolutePath, extracted.absolutePath)
                check(report.startsWith("Extraction complete")) { report }
                val root = DocumentFile.fromTreeUri(this, destinationUri) ?: error("Cannot open destination")
                val target = root.createDirectory("RPF_Extract_" + System.currentTimeMillis()) ?: error("Cannot create output folder")
                copyExtractedTree(extracted, target)
                report + " Output: " + target.name + ". Original archive unchanged."
            } catch (e: Throwable) { "Extraction failed: " + (e.message ?: e.javaClass.simpleName) }
            runOnUiThread { output.text = result }
        }
    }

    private fun copyExtractedTree(source: java.io.File, destination: DocumentFile) {
        val children = source.listFiles() ?: emptyArray()
        for (child in children) {
            if (child.isDirectory) {
                val dir = destination.createDirectory(child.name ?: "folder") ?: error("Cannot create directory")
                copyExtractedTree(child, dir)
            } else if (child.isFile) {
                val file = destination.createFile("application/octet-stream", child.name ?: "asset.bin") ?: error("Cannot create output file")
                contentResolver.openOutputStream(file.uri, "w")?.use { out -> child.inputStream().use { it.copyTo(out) } } ?: error("Cannot write output file")
            }
        }
    }
    private data class HeaderInfo(
        val description: String,
        val entryCount: Long? = null,
        val encryption: String = "Unknown",
        val isGtaV: Boolean = false,
        val assetSummary: String = "TOC not inspected"
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
        val namesLength = bb.getInt(8).toLong() and 0xFFFFFFFFL
        val encryptionWord = bb.getInt(12)
        val encryption = when (encryptionWord) {
            0x4E45504F -> "OPEN (no archive key needed)"
            0x0FFFFFF9 -> "AES encrypted (key material required)"
            0x0FEFFFFF -> "NG encrypted (key material required)"
            0 -> "None"
            else -> "Unknown marker 0x%08X".format(encryptionWord)
        }
        val tocBytes = 16L + entries * 16L + namesLength
        if (entries > 1_000_000L || namesLength > MAX_NAMES_BYTES ||
            tocBytes > MAX_TOC_BYTES || tocBytes < 16L) {
            return HeaderInfo(
                "GTA V RPF7 • invalid or oversized TOC metadata",
                entries, encryption, true, "TOC rejected by safety limits"
            )
        }

        if (encryptionWord != 0x4E45504F && encryptionWord != 0) {
            return HeaderInfo(
                "GTA V RPF7 • encrypted TOC; key required",
                entries, encryption, true, "Asset names unavailable without keys"
            )
        }

        val toc = try {
            contentResolver.openInputStream(uri)?.use { input ->
                val bytes = ByteArray(tocBytes.toInt())
                var offset = 0
                while (offset < bytes.size) {
                    val n = input.read(bytes, offset, bytes.size - offset)
                    if (n <= 0) break
                    offset += n
                }
                if (offset == bytes.size) bytes else null
            }
        } catch (_: Exception) { null } ?: return HeaderInfo(
            "GTA V RPF7 • TOC could not be read",
            entries, encryption, true, "Incomplete TOC read"
        )

        val entryCount = entries.toInt()
        val namesStart = 16 + entryCount * 16
        val names = toc.copyOfRange(namesStart, namesStart + namesLength.toInt())
        val counts = linkedMapOf<String, Int>()
        var badOffsets = 0
        for (i in 0 until entryCount) {
            val off = 16 + i * 16
            val h2 = ByteBuffer.wrap(toc, off + 4, 4).order(ByteOrder.LITTLE_ENDIAN).int
            val nameOffset = if (h2 == 0x7FFFFF00) {
                ByteBuffer.wrap(toc, off, 4).order(ByteOrder.LITTLE_ENDIAN).int.toLong() and 0xFFFFFFFFL
            } else {
                ((toc[off].toInt() and 0xFF) or ((toc[off + 1].toInt() and 0xFF) shl 8)).toLong()
            }
            if (nameOffset >= names.size.toLong()) {
                badOffsets++
                continue
            }
            var nameEnd = nameOffset.toInt()
            while (nameEnd < names.size && names[nameEnd].toInt() != 0) nameEnd++
            if (nameEnd == names.size) {
                badOffsets++
                continue
            }
            val name = names.copyOfRange(nameOffset.toInt(), nameEnd)
                .toString(Charsets.UTF_8).lowercase()
            val ext = name.substringAfterLast('.', "")
            if (ext in ASSET_EXTENSIONS) counts[ext] = (counts[ext] ?: 0) + 1
        }
        val summary = if (counts.isEmpty()) {
            "No recognized asset extensions; invalid name offsets=$badOffsets"
        } else {
            counts.entries.sortedByDescending { it.value }
                .joinToString(", ") { ".${it.key}: ${it.value}" } +
                " • invalid name offsets=$badOffsets"
        }
        return HeaderInfo(
            "GTA V RPF7 • TOC validated",
            entries, encryption, true, summary
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
                        "encryption=${info.encryption} | assets=${info.assetSummary}"
                    )
                }
            }
        }

        rows.sort()
        return buildString {
            appendLine("GTA V RPF INVENTORY — PROTOTYPE 0.3")
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
            appendLine("OPEN/unencrypted RPF7 TOCs are inspected for YTD/YDR/YFT/YMAP and related asset names. AES/NG TOCs need the matching key material.")
            appendLine("RPF7 detection is not proof that the archive's assets are compatible with a specific Android game port.")
            appendLine("This build inventories asset names only; it does not extract, transform or convert assets.")
        }
    }

    override fun onDestroy() {
        worker.shutdown()
        super.onDestroy()
    }

    companion object {
        private const val REQUEST_FOLDER = 1001
        private const val REQUEST_ARCHIVE = 1002
        private const val REQUEST_DESTINATION = 1003
        private const val MAX_NAMES_BYTES = 64L * 1024L * 1024L
        private const val MAX_TOC_BYTES = 80L * 1024L * 1024L
        private val ASSET_EXTENSIONS = setOf("ytd", "ydr", "ydd", "yft", "ybn", "ymap", "ytyp", "ycd", "ysc", "awc", "rel", "dat")
    }
}

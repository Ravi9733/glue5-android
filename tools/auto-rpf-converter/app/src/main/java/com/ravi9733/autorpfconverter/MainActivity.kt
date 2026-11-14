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
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import java.util.concurrent.Executors

class MainActivity : Activity() {
    private val worker = Executors.newSingleThreadExecutor()
    private lateinit var output: TextView
    private var latestReport: String = "No scan has been run yet."
    private var selectedArchiveUri: Uri? = null
    private var selectedArchiveName: String = "selected.rpf"
    private var outputTreeUri: Uri? = null
    private var selectedFolderUri: Uri? = null
    private val rpfKeyDirectory: java.io.File by lazy { java.io.File(filesDir, "rpf-keys") }

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
            text = "Android converter 0.2 • Gen9 resources to Legacy"
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
            text = "SELECT ANDROID RPF OUTPUT FOLDER"
            setOnClickListener {
                val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
                    addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
                }
                startActivityForResult(intent, REQUEST_DESTINATION)
            }
        }
        root.addView(chooseDestination)
        val chooseKeys = Button(this).apply {
            text = "IMPORT OWN GTA V RPF KEYS"
            setOnClickListener {
                val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
                    addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
                }
                startActivityForResult(intent, REQUEST_KEYS)
            }
        }
        root.addView(chooseKeys)
        val extract = Button(this).apply {
            text = "CONVERT SELECTED PC RPF → ANDROID RPF"
            setOnClickListener { extractSelectedArchive() }
        }
        root.addView(extract)
        val bulkExtract = Button(this).apply {
            text = "CONVERT ALL RPF FROM SELECTED GTA V FOLDER"
            setOnClickListener { convertAllOpenArchives() }
        }
        root.addView(bulkExtract)

        root.addView(share)

        output = TextView(this).apply {
            text = "Select a PC RPF or GTA V folder. Import matching AES/NG key files first for encrypted archives. Supported Gen9 YTD, YDR, YDD, YFT and YPT resources are converted to Legacy before a new RPF7 is built."
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
            REQUEST_FOLDER -> {
                selectedFolderUri = uri
                worker.execute {
                    val report = scanTree(uri)
                    runOnUiThread { latestReport = report; output.text = report }
                }
            }
            REQUEST_ARCHIVE -> {
                selectedArchiveUri = uri
                selectedArchiveName = DocumentFile.fromSingleUri(this, uri)?.name ?: "selected.rpf"
                output.text = "Selected " + selectedArchiveName + ". Select output folder next."
            }
            REQUEST_DESTINATION -> {
                outputTreeUri = uri
                output.text = "Output folder selected. Tap CONVERT SELECTED PC RPF → ANDROID RPF."
            }
            REQUEST_KEYS -> {
                output.text = "Importing recognized GTA V key files into app-private storage..."
                worker.execute {
                    val report = try { importRpfKeys(uri) }
                    catch (e: Throwable) { "RPF key import failed: " + (e.message ?: e.javaClass.simpleName) }
                    runOnUiThread { output.text = report }
                }
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
                check(selectedArchiveName.isNotBlank() && !selectedArchiveName.contains('/')) {
                    "Unsafe selected archive filename"
                }
                // NG archive keys depend on the original filename and archive size.
                val source = java.io.File(work, selectedArchiveName)
                copyUriToFile(sourceUri, source, MAX_ARCHIVE_BYTES)
                val extracted = java.io.File(work, "extracted")
                check(extracted.mkdir()) { "Cannot create extraction folder" }
                val extractionReport = NativeAssets.extractArchiveWithKeys(
                    source.absolutePath, extracted.absolutePath, rpfKeyDirectory.absolutePath
                )
                check(extractionReport.startsWith("Extraction complete")) { extractionReport }
                val assetConversionReport = NativeAssets.convertDirectory(extracted.absolutePath)
                check(assetConversionReport.startsWith("Asset conversion complete")) { assetConversionReport }
                val nestedStats = NestedRpfStats()
                convertNestedRpfFiles(extracted, 0, nestedStats)
                val converted = java.io.File(work, safeName.substringBeforeLast('.', safeName) + ".converted.rpf")
                val conversionReport = NativeRpf.buildAndroidRpf7(extracted.absolutePath, converted.absolutePath)
                check(conversionReport.startsWith("RPF7 build complete")) { conversionReport }
                val root = DocumentFile.fromTreeUri(this, destinationUri) ?: error("Cannot open destination")
                val target = createExactOutputFile(root, selectedArchiveName)
                contentResolver.openOutputStream(target.uri, "w")?.use { out ->
                    converted.inputStream().use { input -> input.copyTo(out) }
                } ?: error("Cannot write Android RPF output")
                extractionReport + "\n\n" + assetConversionReport + "\nNested RPFs rebuilt: " + nestedStats.converted + "; preserved/skipped: " + nestedStats.skipped +
                    "\n" + nestedStats.notes.joinToString("\n") + "\n\n" + conversionReport + "\n\nSaved: " + target.name +
                    "\nOriginal PC archive unchanged. Only reported Gen9 formats were converted; other files remain unchanged."
            } catch (e: Throwable) { "Extraction failed: " + (e.message ?: e.javaClass.simpleName) }
            runOnUiThread { output.text = result }
        }
    }


    private fun convertAllOpenArchives() {
        val sourceTree = selectedFolderUri
        val destinationUri = outputTreeUri
        if (sourceTree == null || destinationUri == null) {
            output.text = "First select the GTA V folder, then select an output folder."
            return
        }
        output.text = "Bulk conversion started. OPEN archives need no keys; AES/NG archives need matching imported keys."
        worker.execute {
            val sourceRoot = DocumentFile.fromTreeUri(this, sourceTree)
            val destination = DocumentFile.fromTreeUri(this, destinationUri)
            val report = StringBuilder()
            var done = 0
            var failed = 0
            var skipped = 0
            var totalBytes = 0L
            try {
                check(sourceRoot != null && destination != null) { "Cannot open selected folders" }
                val archives = mutableListOf<Pair<DocumentFile, String>>()
                val stack = ArrayDeque<Pair<DocumentFile, String>>()
                var scannedItems = 0
                stack.add(sourceRoot!! to "")
                while (stack.isNotEmpty()) {
                    val pair = stack.removeLast()
                    val dir = pair.first
                    val relativeDir = pair.second
                    for (child in dir.listFiles()) {
                        scannedItems++
                        if (scannedItems > MAX_FOLDER_ITEMS) {
                            error("Selected folder tree exceeds the " + MAX_FOLDER_ITEMS + " item safety limit.")
                        }
                        val childName = child.name ?: continue
                        val relative = if (relativeDir.isEmpty()) childName else relativeDir + "/" + childName
                        if (child.isDirectory) {
                            stack.add(child to relative)
                        } else if (child.isFile && childName.endsWith(".rpf", true)) {
                            archives.add(child to relativeDir)
                            if (archives.size > MAX_BULK_ARCHIVES) {
                                error("Found more than " + MAX_BULK_ARCHIVES + " RPF files; select a smaller folder.")
                            }
                        }
                    }
                }
                report.appendLine("RPF archives discovered: " + archives.size)
                for ((index, pair) in archives.withIndex()) {
                    val archive = pair.first
                    val relativeDir = pair.second
                    val name = archive.name ?: ("archive_" + index + ".rpf")
                    try {
                        val info = inspectHeader(archive.uri)
                        if (!info.isGtaV || !(info.encryption.startsWith("OPEN") ||
                                info.encryption == "None" || info.encryption.startsWith("AES encrypted") ||
                                info.encryption.startsWith("NG encrypted"))) {
                            skipped++
                            report.appendLine("SKIP " + name + " — unsupported RPF7 encryption marker: " + info.encryption)
                            continue
                        }
                        if (archive.length() > MAX_ARCHIVE_BYTES) {
                            skipped++
                            report.appendLine("SKIP " + name + " — archive exceeds the 1 GiB input limit")
                            continue
                        }
                        val work = java.io.File(cacheDir, "bulk-rpf-work")
                        work.deleteRecursively()
                        check(work.mkdirs()) { "Cannot create temporary folder" }
                        val safeName = name.replace(Regex("[^A-Za-z0-9._-]"), "_")
                        check(name.isNotBlank() && !name.contains('/')) { "Unsafe source archive filename" }
                        // Keep the real basename: NG decryption selects a key from filename + size.
                        val source = java.io.File(work, name)
                        copyUriToFile(archive.uri, source, MAX_ARCHIVE_BYTES)
                        val extracted = java.io.File(work, "extracted")
                        check(extracted.mkdir()) { "Cannot create extraction folder" }
                        val extractionReport = NativeAssets.extractArchiveWithKeys(
                    source.absolutePath, extracted.absolutePath, rpfKeyDirectory.absolutePath
                )
                        check(extractionReport.startsWith("Extraction complete")) { extractionReport }
                        val assetReport = NativeAssets.convertDirectory(extracted.absolutePath)
                        check(assetReport.startsWith("Asset conversion complete")) { assetReport }
                        val nestedStats = NestedRpfStats()
                        convertNestedRpfFiles(extracted, 0, nestedStats)
                        val converted = java.io.File(work, safeName.substringBeforeLast('.', safeName) + ".converted.rpf")
                        val packageReport = NativeRpf.buildAndroidRpf7(extracted.absolutePath, converted.absolutePath)
                        check(packageReport.startsWith("RPF7 build complete")) { packageReport }

                        val outputDirectory = ensureOutputDirectory(destination!!, relativeDir)
                        val target = createExactOutputFile(outputDirectory, name)
                        contentResolver.openOutputStream(target.uri, "w")?.use { out ->
                            converted.inputStream().use { input -> input.copyTo(out) }
                        } ?: error("Cannot write Android RPF output")
                        done++
                        totalBytes += converted.length()
                        report.appendLine("OK " + relativeDir + "/" + name + " → " + relativeDir + "/" + target.name)
                        report.appendLine(assetReport.lineSequence().filter {
                            it.startsWith("Gen9 to Legacy") || it.startsWith("Already Legacy")
                        }.joinToString(" | "))
                        report.appendLine("Nested RPFs rebuilt=" + nestedStats.converted + ", preserved/skipped=" + nestedStats.skipped)
                        nestedStats.notes.forEach { report.appendLine(it) }
                    } catch (e: Throwable) {
                        failed++
                        report.appendLine("FAIL " + relativeDir + "/" + name + " — " + (e.message ?: e.javaClass.simpleName))
                    }
                    val status = "Bulk conversion " + (index + 1) + "/" + archives.size +
                        "\nSuccess: " + done + " • Failed: " + failed + " • Skipped: " + skipped
                    runOnUiThread { output.text = status + "\n\n" + report.toString().takeLast(12000) }
                }
                report.appendLine()
                report.appendLine("Finished. Converted/repacked=" + done + ", Failed=" + failed + ", Skipped=" + skipped)
                report.appendLine("Output bytes written: " + totalBytes)
                report.appendLine("Source archives were not modified.")
                report.appendLine("Nested RPF file entries are rebuilt recursively up to the configured depth and RPF7 size limits.")
            } catch (e: Throwable) {
                report.appendLine("Bulk operation stopped: " + (e.message ?: e.javaClass.simpleName))
            }
            runOnUiThread { latestReport = report.toString(); output.text = latestReport }
        }
    }

    private data class NestedRpfStats(
        var seen: Int = 0,
        var converted: Int = 0,
        var skipped: Int = 0,
        val notes: MutableList<String> = mutableListOf()
    )

    private fun convertNestedRpfFiles(root: java.io.File, depth: Int, stats: NestedRpfStats) {
        val nestedFiles = root.walkTopDown()
            .filter { it.isFile && it.extension.equals("rpf", ignoreCase = true) }
            .toList()
        for (nested in nestedFiles) {
            stats.seen++
            if (stats.seen > MAX_NESTED_ARCHIVES) {
                error("Nested RPF count exceeds the " + MAX_NESTED_ARCHIVES + " archive limit")
            }
            if (depth >= MAX_NESTED_DEPTH) {
                stats.skipped++
                stats.notes.add("PRESERVED " + nested.name + " — nested depth limit reached")
                continue
            }
            if (Files.isSymbolicLink(nested.toPath())) {
                error("Symbolic link rejected in nested RPF tree: " + nested.absolutePath)
            }
            if (nested.length() > MAX_NESTED_RPF_BYTES) {
                error("Nested RPF " + nested.name + " exceeds the RPF7 24-bit binary-entry limit (16 MiB); refusing to truncate it")
            }

            val work = java.io.File(cacheDir, "nested-rpf-work-" + stats.seen)
            work.deleteRecursively()
            check(work.mkdirs()) { "Cannot create nested RPF work folder" }
            val extracted = java.io.File(work, "extracted")
            check(extracted.mkdir()) { "Cannot create nested extraction folder" }

            val extractionReport = NativeAssets.extractArchiveWithKeys(
                nested.absolutePath, extracted.absolutePath, rpfKeyDirectory.absolutePath
            )
            if (!extractionReport.startsWith("Extraction complete")) {
                stats.skipped++
                stats.notes.add("PRESERVED " + nested.name + " — " + extractionReport.replace('\n', ' '))
                work.deleteRecursively()
                continue
            }

            val assetReport = NativeAssets.convertDirectory(extracted.absolutePath)
            check(assetReport.startsWith("Asset conversion complete")) { assetReport }
            convertNestedRpfFiles(extracted, depth + 1, stats)

            val converted = java.io.File(work, "nested_android.rpf")
            val packageReport = NativeRpf.buildAndroidRpf7(extracted.absolutePath, converted.absolutePath)
            check(packageReport.startsWith("RPF7 build complete")) { packageReport }
            if (converted.length() > MAX_NESTED_RPF_BYTES) {
                error("Converted nested RPF " + nested.name + " exceeds 16 MiB; refusing to truncate it")
            }

            val replacement = java.io.File(nested.parentFile, nested.name + ".converted")
            if (replacement.exists()) replacement.delete()
            converted.copyTo(replacement, overwrite = true)
            Files.move(replacement.toPath(), nested.toPath(), StandardCopyOption.REPLACE_EXISTING)
            stats.converted++
            stats.notes.add("REBUILT " + nested.name + " — supported Gen9 resources converted to Legacy")
            work.deleteRecursively()
        }
    }

    private fun importRpfKeys(uri: Uri): String {
        val source = DocumentFile.fromTreeUri(this, uri) ?: error("Cannot open selected key folder")
        rpfKeyDirectory.deleteRecursively()
        check(rpfKeyDirectory.mkdirs()) { "Cannot create app-private RPF key folder" }
        var copied = 0
        for ((name, expectedBytes) in RPF_KEY_FILE_SIZES) {
            val document = source.findFile(name) ?: continue
            check(document.isFile) { name + " is not a file" }
            val target = java.io.File(rpfKeyDirectory, name)
            copyUriToFile(document.uri, target, expectedBytes)
            check(target.length() == expectedBytes) {
                name + " has an invalid size: expected " + expectedBytes + " bytes, got " + target.length()
            }
            copied++
        }
        check(copied > 0) {
            "No recognized key files found. Select the folder containing gtav_aes_key.dat and/or the ragekit NG key files."
        }
        return NativeAssets.inspectRpfKeys(rpfKeyDirectory.absolutePath)
    }

    private fun copyUriToFile(uri: Uri, target: java.io.File, maxBytes: Long) {
        contentResolver.openInputStream(uri)?.use { input ->
            java.io.FileOutputStream(target).use { out ->
                val buffer = ByteArray(64 * 1024)
                var total = 0L
                while (true) {
                    val count = input.read(buffer)
                    if (count < 0) break
                    total += count
                    if (total > maxBytes) error("Archive exceeds the " + (maxBytes / (1024 * 1024)) + " MiB copy limit")
                    out.write(buffer, 0, count)
                }
                out.fd.sync()
            }
        } ?: error("Cannot open selected archive")
    }

    private fun ensureOutputDirectory(root: DocumentFile, relativePath: String): DocumentFile {
        var current = root
        if (relativePath.isBlank()) return current
        for (part in relativePath.split('/')) {
            check(part.isNotBlank() && part != "." && part != ".." && !part.contains('/')) {
                "Unsafe source folder name cannot be mirrored: " + part
            }
            val existing = current.findFile(part)
            current = if (existing != null && existing.isDirectory) {
                existing
            } else if (existing == null) {
                current.createDirectory(part) ?: error("Cannot create output directory " + part)
            } else {
                error("Output path component is already a file: " + part)
            }
        }
        return current
    }

    private fun createExactOutputFile(directory: DocumentFile, requestedName: String): DocumentFile {
        check(requestedName.isNotBlank() && requestedName != "." && requestedName != ".." && !requestedName.contains('/')) {
            "Unsafe RPF filename cannot be preserved: " + requestedName
        }
        check(directory.findFile(requestedName) == null) {
            "Output already contains " + requestedName + ". Choose an empty output folder to preserve DLC filenames and avoid overwriting."
        }
        return directory.createFile("application/octet-stream", requestedName)
            ?: error("Cannot create Android RPF output with original filename " + requestedName)
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
        var visitedItems = 0
        var scanLimitReached = false
        var totalBytes = 0L
        var gtaVArchives = 0
        var openArchives = 0
        var encryptedArchives = 0
        var unreadable = 0
        val stack = ArrayDeque<DocumentFile>()
        stack.add(root)

        while (stack.isNotEmpty() && !scanLimitReached) {
            val dir = stack.removeLast()
            val children = try { dir.listFiles() } catch (_: Exception) { emptyArray() }
            for (child in children) {
                visitedItems++
                if (visitedItems > MAX_FOLDER_ITEMS) {
                    scanLimitReached = true
                    break
                }
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
            appendLine("GTA V RPF INVENTORY — ANDROID CONVERTER 0.2")
            appendLine("RPF files found: ${rows.size}")
            appendLine("GTA V RPF7 archives: $gtaVArchives")
            appendLine("OPEN archives: $openArchives")
            appendLine("AES/NG encrypted archives: $encryptedArchives")
            appendLine("Unreadable headers: $unreadable")
            appendLine("Total RPF size: ${"%.2f".format(totalBytes / 1073741824.0)} GiB")
            appendLine()
            rows.forEach { appendLine(it) }
            if (scanLimitReached) appendLine("Scan truncated at the " + MAX_FOLDER_ITEMS + " item safety limit.")
            appendLine()
            appendLine("Read-only scan. No source files were changed.")
            appendLine("OPEN/unencrypted RPF7 TOCs are inspected for YTD/YDR/YFT/YMAP and related asset names. AES/NG TOCs need the matching key material.")
            appendLine("RPF7 structure and resource conversion do not prove that the result will mount or run in a specific Android game port such as Glue5.")
            appendLine("Bulk conversion is available for OPEN RPF7 archives. Supported Gen9 YTD/YDR/YDD/YFT/YPT resources are converted to Legacy before RPF7 repacking; other formats remain unchanged.")
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
        private const val REQUEST_KEYS = 1004
        private val RPF_KEY_FILE_SIZES = linkedMapOf(
            "gtav_aes_key.dat" to 32L,
            "gtav_ng_key.dat" to 101L * 272L,
            "gtav_ng_decrypt_tables.dat" to 17L * 16L * 256L * 4L,
            "gtav_hash_lut.dat" to 256L,
            "gtav_awc_key.dat" to 16L
        )
        private const val MAX_NAMES_BYTES = 64L * 1024L * 1024L
        private const val MAX_TOC_BYTES = 80L * 1024L * 1024L
        private const val MAX_BULK_ARCHIVES = 500
        private const val MAX_FOLDER_ITEMS = 250_000
        private const val MAX_ARCHIVE_BYTES = 1024L * 1024L * 1024L
        private const val MAX_NESTED_RPF_BYTES = 0xFFFFFFL
        private const val MAX_NESTED_DEPTH = 8
        private const val MAX_NESTED_ARCHIVES = 500
        private val ASSET_EXTENSIONS = setOf("ytd", "ydr", "ydd", "yft", "ybn", "ymap", "ytyp", "ycd", "ysc", "awc", "rel", "dat")
    }
}

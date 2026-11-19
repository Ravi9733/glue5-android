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
import java.security.MessageDigest
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
            text = "Android converter 0.3.7 • direct RPF7 conversion + platform DLC audit"
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
            text = "CONVERT SELECTED PC RPF → OPEN RPF7"
            setOnClickListener { extractSelectedArchive() }
        }
        root.addView(extract)
        val validateDlc = Button(this).apply {
            text = "VALIDATE SELECTED DLC.RPF METADATA"
            setOnClickListener { inspectSelectedDlcPack() }
        }
        root.addView(validateDlc)
        val validateLoadOrder = Button(this).apply {
            text = "VALIDATE FULL DLC LOAD ORDER"
            setOnClickListener { inspectFullDlcLoadOrder() }
        }
        root.addView(validateLoadOrder)
        val bulkExtract = Button(this).apply {
            text = "CONVERT ALL RPF FROM SELECTED GTA V FOLDER"
            setOnClickListener { convertAllOpenArchives() }
        }
        root.addView(bulkExtract)

        root.addView(share)

        output = TextView(this).apply {
            text = "Select a PC RPF or GTA V folder. Import matching AES/NG key files first for encrypted archives. The converter reads RPF entries directly, converts supported Gen9 YTD/YDR/YDD/YFT/YPT resources to Legacy, validates and preserves YBN/YMAP/YTYP, and rebuilds nested archives as OPEN RPF7. Unsupported files and encrypted .ysc stored bytes are preserved unchanged. This output is not certified as Glue5 prepared game data; game.wasm and translated shaders are separate requirements."
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
                output.text = "Output folder selected. Tap CONVERT SELECTED PC RPF → OPEN RPF7."
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
        output.text = "Converting the RPF directly. Keep the app open; this can take time."
        worker.execute {
            val work = java.io.File(cacheDir, "rpf-work")
            val result = try {
                TemporaryWorkspacePolicy.prepare(work)
                val safeName = selectedArchiveName.replace(Regex("[^A-Za-z0-9._-]"), "_")
                check(selectedArchiveName.isNotBlank() && !selectedArchiveName.contains('/')) {
                    "Unsafe selected archive filename"
                }
                // Keep the real basename: NG archive key selection depends on filename and archive size.
                val source = java.io.File(work, selectedArchiveName)
                copyUriToFile(sourceUri, source, MAX_ARCHIVE_BYTES)
                val converted = java.io.File(work, safeName.substringBeforeLast('.', safeName) + ".converted.rpf")
                val conversionReport = NativeAssets.convertArchiveToOpenRpf7(
                    source.absolutePath, converted.absolutePath, rpfKeyDirectory.absolutePath
                )
                check(conversionReport.startsWith("RPF7 conversion complete")) { conversionReport }
                val root = DocumentFile.fromTreeUri(this, destinationUri) ?: error("Cannot open destination")
                val target = writeOutputFileSafely(converted, root, selectedArchiveName)
                conversionReport + "\n\nSaved: " + target.name +
                    "\nOriginal PC archive unchanged. Only reported Gen9 resources were converted; unsupported files remain unchanged."
            } catch (e: Throwable) {
                "Conversion failed: " + (e.message ?: e.javaClass.simpleName)
            } finally {
                // RPF input and generated output can each be several GiB. Do not leave
                // either copy in cache after the SAF export has been verified.
                TemporaryWorkspacePolicy.clean(work)
            }
            runOnUiThread { output.text = result }
        }
    }



    private fun inspectSelectedDlcPack() {
        val sourceUri = selectedArchiveUri
        val selectedName = selectedArchiveName
        if (sourceUri == null) {
            output.text = "Select a DLC pack's dlc.rpf archive first."
            return
        }
        if (!selectedName.equals("dlc.rpf", ignoreCase = true)) {
            output.text = "Choose the dlc.rpf file inside the DLC pack folder, not update.rpf or another archive."
            return
        }

        output.text = "Reading setup2.xml and its declared content metadata. The source DLC archive will not be changed."
        worker.execute {
            val work = java.io.File(cacheDir, "dlc-metadata-work")
            val report = try {
                TemporaryWorkspacePolicy.prepare(work)
                val source = java.io.File(work, "dlc.rpf")
                copyUriToFile(sourceUri, source, MAX_ARCHIVE_BYTES)
                NativeAssets.inspectDlcPackMetadata(source.absolutePath, rpfKeyDirectory.absolutePath)
            } catch (e: Throwable) {
                "DLC metadata validation failed: " + (e.message ?: e.javaClass.simpleName)
            } finally {
                TemporaryWorkspacePolicy.clean(work)
            }
            runOnUiThread {
                latestReport = report
                output.text = report
            }
        }
    }


    private fun inspectFullDlcLoadOrder() {
        val sourceTree = selectedFolderUri
        if (sourceTree == null) {
            output.text = "Select the GTA V root folder first."
            return
        }
        output.text = "Reading update.rpf/dlclist.xml and checking registered DLC packs. This is read-only."
        worker.execute {
            val report = StringBuilder()
            val work = java.io.File(cacheDir, "dlc-load-order-work")
            var registered = 0
            var validated = 0
            var missing = 0
            var failed = 0
            var duplicates = 0
            var platform = 0
            val platformPacks = mutableListOf<Pair<Int, String>>()
            var unsafe = 0
            var unsupported = 0
            var totalBytes = 0L
            try {
                val sourceRoot = DocumentFile.fromTreeUri(this, sourceTree)
                    ?: error("Cannot open selected GTA V folder")
                val modsUpdate = findDocumentPath(sourceRoot, listOf("mods", "update", "update.rpf"))
                val baseUpdate = findDocumentPath(sourceRoot, listOf("update", "update.rpf"))
                val updateDocument = modsUpdate ?: baseUpdate
                    ?: error("Cannot find mods/update/update.rpf or update/update.rpf")
                check(updateDocument.isFile) { "Selected update.rpf path is not a file" }
                val updatePathLabel = if (modsUpdate != null) "mods/update/update.rpf" else "update/update.rpf"

                TemporaryWorkspacePolicy.prepare(work)
                val updateCopy = java.io.File(work, "update.rpf")
                copyUriToFile(updateDocument.uri, updateCopy, MAX_ARCHIVE_BYTES)
                totalBytes = updateCopy.length()
                check(totalBytes <= MAX_DLC_AUDIT_TOTAL_BYTES) { "update.rpf exceeds the DLC audit byte budget" }

                val listReport = NativeAssets.readDlcListEntries(
                    updateCopy.absolutePath, rpfKeyDirectory.absolutePath
                )
                check(listReport.startsWith("DLC_LIST_V1")) { listReport }
                val entries = listReport.lineSequence()
                    .filter { it.contains('|') }
                    .map { it.substringBefore('|') to it.substringAfter('|') }
                    .toList()

                report.appendLine("GTA V DLC LOAD-ORDER AUDIT — READ ONLY")
                report.appendLine("List source: " + updatePathLabel + "/common/data/dlclist.xml")
                report.appendLine("Entries in dlclist.xml: " + entries.size)
                report.appendLine("Order below follows dlclist.xml; setup2.xml order is shown separately.")
                report.appendLine()

                val seen = mutableSetOf<String>()
                val seenPlatform = mutableSetOf<String>()
                var budgetReached = false
                for ((position, item) in entries.withIndex()) {
                    if (budgetReached) break
                    val kind = item.first
                    val value = item.second
                    when (kind) {
                        "DLC" -> {
                            registered++
                            if (registered > MAX_DLC_AUDIT_PACKS) {
                                report.appendLine("STOP — 512 DLC pack audit limit reached.")
                                budgetReached = true
                                break
                            }
                            if (!seen.add(value.lowercase())) {
                                duplicates++
                                report.appendLine("DUPLICATE [" + (position + 1) + "] " + value)
                                continue
                            }
                            if (value.isBlank() || value == "." || value == ".." ||
                                value.contains('/') || value.contains('\\') || value.contains(':')) {
                                unsafe++
                                report.appendLine("UNSAFE [" + (position + 1) + "] invalid pack name")
                                continue
                            }

                            var packDocument: DocumentFile? = null
                            var packPath = ""
                            val candidates = listOf(
                                Pair(listOf("mods", "update", "x64", "dlcpacks", value, "dlc.rpf"),
                                    "mods/update/x64/dlcpacks/" + value + "/dlc.rpf"),
                                Pair(listOf("update", "x64", "dlcpacks", value, "dlc.rpf"),
                                    "update/x64/dlcpacks/" + value + "/dlc.rpf")
                            )
                            for (candidate in candidates) {
                                val found = findDocumentPath(sourceRoot, candidate.first)
                                if (found != null) {
                                    packDocument = found
                                    packPath = candidate.second
                                    break
                                }
                            }
                            if (packDocument == null || !packDocument.isFile) {
                                missing++
                                report.appendLine("MISSING [" + (position + 1) + "] " + value +
                                    " — no dlc.rpf under update/x64/dlcpacks or mods/update/x64/dlcpacks")
                                continue
                            }

                            val sizeHint = packDocument.length()
                            if (sizeHint > MAX_ARCHIVE_BYTES) {
                                failed++
                                report.appendLine("FAIL [" + (position + 1) + "] " + value + " — archive exceeds 4 GiB limit")
                                continue
                            }
                            if (sizeHint > 0 && totalBytes > MAX_DLC_AUDIT_TOTAL_BYTES - sizeHint) {
                                report.appendLine("STOP — 16 GiB cumulative DLC audit read budget reached.")
                                budgetReached = true
                                break
                            }

                            val packWork = java.io.File(work, "pack-" + (position + 1))
                            TemporaryWorkspacePolicy.prepare(packWork)
                            try {
                                val packCopy = java.io.File(packWork, "dlc.rpf")
                                val remainingBudget = MAX_DLC_AUDIT_TOTAL_BYTES - totalBytes
                                copyUriToFile(packDocument.uri, packCopy, minOf(MAX_ARCHIVE_BYTES, remainingBudget))
                                val copiedBytes = packCopy.length()
                                check(copiedBytes <= remainingBudget) {
                                    "DLC archive copy exceeded the remaining cumulative audit budget"
                                }
                                totalBytes += copiedBytes
                                val packReport = NativeAssets.inspectDlcPackMetadata(
                                    packCopy.absolutePath, rpfKeyDirectory.absolutePath
                                )
                                if (!packReport.startsWith("DLC metadata validation complete")) {
                                    failed++
                                    report.appendLine("FAIL [" + (position + 1) + "] " + value + " — " +
                                        packReport.lineSequence().firstOrNull().orEmpty())
                                } else {
                                    validated++
                                    report.appendLine("OK [" + (position + 1) + "] " + value +
                                        " | source=" + packPath +
                                        " | device=" + dlcReportField(packReport, "Device name") +
                                        " | setup2 order=" + dlcReportField(packReport, "Load order") +
                                        " | data refs=" + dlcReportField(packReport, "Data files resolved") +
                                        "/" + dlcReportField(packReport, "Data files declared") +
                                        " | missing=" + dlcReportField(packReport, "Missing references") +
                                        " | unsafe=" + dlcReportField(packReport, "Unsafe references") +
                                        " | RPF kind mismatch=" + dlcReportField(packReport, "RPF_FILE type mismatches"))
                                }
                            } catch (e: Throwable) {
                                if ((e.message ?: "").contains("copy limit")) {
                                    budgetReached = true
                                    report.appendLine("STOP — cumulative 16 GiB audit byte budget reached while copying " + value)
                                    break
                                }
                                failed++
                                report.appendLine("FAIL [" + (position + 1) + "] " + value + " — " +
                                    (e.message ?: e.javaClass.simpleName))
                            } finally {
                                TemporaryWorkspacePolicy.clean(packWork)
                            }
                            val progress = "DLC audit " + (position + 1) + "/" + entries.size +
                                " | validated=" + validated + " missing=" + missing + " failed=" + failed
                            runOnUiThread { output.text = progress }
                        }
                        "PLATFORM" -> {
                            platform++
                            if (!seenPlatform.add(value.lowercase())) {
                                duplicates++
                                report.appendLine("DUPLICATE PLATFORM [" + (position + 1) + "] " + value)
                            } else {
                                platformPacks.add((position + 1) to value)
                                report.appendLine("PLATFORM [" + (position + 1) + "] " + value +
                                    " — queued for read-only lookup in base x64*.rpf archives")
                            }
                        }
                        "UNSAFE" -> {
                            unsafe++
                            report.appendLine("UNSAFE [" + (position + 1) + "] traversal or invalid pack path")
                        }
                        else -> {
                            unsupported++
                            report.appendLine("UNSUPPORTED [" + (position + 1) + "] unrecognized dlclist mount path")
                        }
                    }
                }

                var platformValidated = 0
                var platformFailed = 0
                var platformLocked = 0
                var platformMissing = 0
                var platformUnresolved = 0
                var unreadableBaseArchives = 0
                var readableBaseArchives = 0

                if (platformPacks.isNotEmpty()) {
                    report.appendLine()
                    report.appendLine("Resolving platform packs by scanning x64w.rpf through x64a.rpf.")
                    report.appendLine("Expected internal path: dlcpacks/<name>/dlc.rpf")
                    val unresolved = platformPacks.toMutableList()
                    for (letter in 'w' downTo 'a') {
                        if (unresolved.isEmpty() || budgetReached) break
                        val archiveName = "x64" + letter + ".rpf"
                        val baseDocument = findDocumentPath(sourceRoot, listOf(archiveName))
                            ?: continue
                        if (!baseDocument.isFile) continue

                        val sizeHint = baseDocument.length()
                        if (sizeHint > MAX_ARCHIVE_BYTES) {
                            unreadableBaseArchives++
                            report.appendLine("UNREADABLE " + archiveName + " — exceeds the 4 GiB archive limit")
                            continue
                        }
                        if (sizeHint > 0 && totalBytes > MAX_DLC_AUDIT_TOTAL_BYTES - sizeHint) {
                            report.appendLine("STOP — cumulative 16 GiB audit byte budget reached before " + archiveName)
                            budgetReached = true
                            break
                        }

                        val baseCopy = java.io.File(work, archiveName)
                        try {
                            val remainingBudget = MAX_DLC_AUDIT_TOTAL_BYTES - totalBytes
                            copyUriToFile(baseDocument.uri, baseCopy, minOf(MAX_ARCHIVE_BYTES, remainingBudget))
                            val copiedBytes = baseCopy.length()
                            check(copiedBytes <= remainingBudget) {
                                "Base archive copy exceeded the remaining cumulative audit budget"
                            }
                            totalBytes += copiedBytes

                            var baseReadable = false
                            var baseError: String? = null
                            for (packItem in unresolved.toList()) {
                                val packPosition = packItem.first
                                val packName = packItem.second
                                val platformReport = NativeAssets.inspectPlatformDlcPackMetadata(
                                    baseCopy.absolutePath, packName, rpfKeyDirectory.absolutePath
                                )
                                when {
                                    platformReport.startsWith("PLATFORM_DLC_NOT_FOUND\n") -> {
                                        baseReadable = true
                                    }
                                    platformReport.startsWith("PLATFORM_DLC_FOUND\n") -> {
                                        baseReadable = true
                                        platformValidated++
                                        unresolved.remove(packItem)
                                        report.appendLine("PLATFORM OK [" + packPosition + "] " + packName +
                                            " | base=" + archiveName +
                                            " | device=" + dlcReportField(platformReport, "Device name") +
                                            " | setup2 order=" + dlcReportField(platformReport, "Load order") +
                                            " | data refs=" + dlcReportField(platformReport, "Data files resolved") +
                                            "/" + dlcReportField(platformReport, "Data files declared") +
                                            " | missing=" + dlcReportField(platformReport, "Missing references"))
                                    }
                                    platformReport.startsWith("PLATFORM_DLC_FOUND_LOCKED\n") -> {
                                        baseReadable = true
                                        platformLocked++
                                        unresolved.remove(packItem)
                                        report.appendLine("PLATFORM FOUND BUT KEY-LOCKED [" + packPosition + "] " +
                                            packName + " | base=" + archiveName +
                                            " | import matching legitimate AES/NG key material to validate")
                                    }
                                    platformReport.startsWith("PLATFORM_DLC_FOUND_INVALID\n") -> {
                                        baseReadable = true
                                        platformFailed++
                                        unresolved.remove(packItem)
                                        report.appendLine("PLATFORM FOUND BUT INVALID [" + packPosition + "] " +
                                            packName + " | base=" + archiveName + " | " +
                                            (platformReport.lineSequence().firstOrNull { it.startsWith("Metadata inspection failed:") }
                                                ?: "nested DLC metadata validation failed"))
                                    }
                                    platformReport.startsWith("PLATFORM_DLC_INVALID_ENTRY\n") -> {
                                        baseReadable = true
                                        platformFailed++
                                        unresolved.remove(packItem)
                                        report.appendLine("PLATFORM INVALID ENTRY [" + packPosition + "] " +
                                            packName + " | base=" + archiveName)
                                    }
                                    else -> {
                                        baseError = platformReport.lineSequence().firstOrNull()
                                            ?: "unknown native error"
                                        break
                                    }
                                }
                            }
                            if (baseReadable) readableBaseArchives++
                            if (baseError != null) {
                                unreadableBaseArchives++
                                report.appendLine("UNREADABLE " + archiveName + " — " + baseError +
                                    " | unresolved packs will not be marked missing")
                            }
                        } catch (e: Throwable) {
                            if ((e.message ?: "").contains("copy limit")) {
                                budgetReached = true
                                report.appendLine("STOP — cumulative 16 GiB audit byte budget reached while copying " + archiveName)
                                break
                            }
                            unreadableBaseArchives++
                            report.appendLine("UNREADABLE " + archiveName + " — " +
                                (e.message ?: e.javaClass.simpleName))
                        } finally {
                            baseCopy.delete()
                        }
                    }

                    for ((packPosition, packName) in unresolved) {
                        if (budgetReached) {
                            platformUnresolved++
                            report.appendLine("UNRESOLVED PLATFORM [" + packPosition + "] " + packName +
                                " — audit stopped at the 16 GiB safety budget")
                        } else if (unreadableBaseArchives > 0 || readableBaseArchives == 0) {
                            platformUnresolved++
                            report.appendLine("UNKNOWN PLATFORM [" + packPosition + "] " + packName +
                                " — at least one base archive was unreadable or no x64 archive was available; not marked missing")
                        } else {
                            platformMissing++
                            report.appendLine("MISSING PLATFORM [" + packPosition + "] " + packName +
                                " — not found in the readable x64w.rpf through x64a.rpf archives")
                        }
                    }
                }

                report.appendLine()
                report.appendLine("Registered dlcpacks entries: " + registered)
                report.appendLine("Pack metadata validated: " + validated)
                report.appendLine("Missing DLC archives: " + missing)
                report.appendLine("Unreadable/invalid DLC archives: " + failed)
                report.appendLine("Duplicate list entries: " + duplicates)
                report.appendLine("Platform references in dlclist.xml: " + platform)
                report.appendLine("Platform pack metadata validated: " + platformValidated)
                report.appendLine("Platform packs found but invalid: " + platformFailed)
                report.appendLine("Platform packs found but key-locked: " + platformLocked)
                report.appendLine("Missing platform pack archives: " + platformMissing)
                report.appendLine("Platform references unresolved: " + platformUnresolved)
                report.appendLine("Unreadable base x64 archives: " + unreadableBaseArchives)
                report.appendLine("Readable base x64 archives scanned: " + readableBaseArchives)
                report.appendLine("Unsafe list entries: " + unsafe)
                report.appendLine("Unsupported list entries: " + unsupported)
                report.appendLine("Bytes read: " + totalBytes)
                if (budgetReached) report.appendLine("Audit stopped at the cumulative 16 GiB safety budget.")
                report.appendLine("Source update.rpf, base x64 archives and DLC packs were not modified.")
                report.appendLine("Platform lookup is read-only and searches dlcpacks/<name>/dlc.rpf inside x64w.rpf through x64a.rpf.")
                report.appendLine("This audit does not read the separate common.rpf dlclist.xml or prove Glue5 mounts the packs.")
            } catch (e: Throwable) {
                report.appendLine("DLC load-order audit failed: " + (e.message ?: e.javaClass.simpleName))
                report.appendLine("Source game files were not modified.")
            } finally {
                TemporaryWorkspacePolicy.clean(work)
            }
            runOnUiThread {
                latestReport = report.toString()
                output.text = latestReport
            }
        }
    }

    private fun findDocumentPath(root: DocumentFile, parts: List<String>): DocumentFile? {
        var current = root
        for ((index, part) in parts.withIndex()) {
            val next = current.listFiles().firstOrNull {
                it.name?.equals(part, ignoreCase = true) == true
            } ?: return null
            if (index < parts.lastIndex && !next.isDirectory) return null
            current = next
        }
        return current
    }

    private fun dlcReportField(report: String, field: String): String {
        val prefix = field + ":"
        return report.lineSequence().firstOrNull { it.startsWith(prefix) }
            ?.substringAfter(':')?.trim() ?: "?"
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
            val work = java.io.File(cacheDir, "bulk-rpf-work")
            try {
                TemporaryWorkspacePolicy.prepare(work)
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
                            report.appendLine("SKIP " + name + " — archive exceeds the 4 GiB input limit")
                            continue
                        }
                        TemporaryWorkspacePolicy.prepare(work)
                        val safeName = name.replace(Regex("[^A-Za-z0-9._-]"), "_")
                        check(name.isNotBlank() && !name.contains('/')) { "Unsafe source archive filename" }
                        // Keep the real basename: NG decryption selects a key from filename + size.
                        val source = java.io.File(work, name)
                        copyUriToFile(archive.uri, source, MAX_ARCHIVE_BYTES)
                        val converted = java.io.File(work, safeName.substringBeforeLast('.', safeName) + ".converted.rpf")
                        val packageReport = NativeAssets.convertArchiveToOpenRpf7(
                            source.absolutePath, converted.absolutePath, rpfKeyDirectory.absolutePath
                        )
                        check(packageReport.startsWith("RPF7 conversion complete")) { packageReport }

                        val outputDirectory = ensureOutputDirectory(destination!!, relativeDir)
                        val target = writeOutputFileSafely(converted, outputDirectory, name)
                        done++
                        totalBytes += converted.length()
                        report.appendLine("OK " + relativeDir + "/" + name + " → " + relativeDir + "/" + target.name)
                        report.appendLine(packageReport.lineSequence().filter {
                            it.startsWith("Gen9 to Legacy") || it.startsWith("Edition-independent resources") ||
                                it.startsWith("Already Legacy") || it.startsWith("Other files preserved") ||
                                it.startsWith("Nested RPFs rebuilt")
                        }.joinToString(" | "))
                    } catch (e: Throwable) {
                        failed++
                        report.appendLine("FAIL " + relativeDir + "/" + name + " — " + (e.message ?: e.javaClass.simpleName))
                    } finally {
                        // Release the source copy and generated RPF before processing
                        // the next archive; large DLC trees must not accumulate in cache.
                        TemporaryWorkspacePolicy.clean(work)
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
            } finally {
                TemporaryWorkspacePolicy.clean(work)
            }
            runOnUiThread { latestReport = report.toString(); output.text = latestReport }
        }
    }

    private fun importRpfKeys(uri: Uri): String {
        val source = DocumentFile.fromTreeUri(this, uri) ?: error("Cannot open selected key folder")
        val staging = java.io.File(filesDir, "rpf-keys.importing")
        val backup = java.io.File(filesDir, "rpf-keys.previous")
        check(!Files.isSymbolicLink(rpfKeyDirectory.toPath()) &&
            !Files.isSymbolicLink(staging.toPath()) && !Files.isSymbolicLink(backup.toPath())) {
            "Unexpected symbolic link in app-private key storage"
        }
        // Recover a previous key set if the process was interrupted between directory renames.
        if (!rpfKeyDirectory.exists() && backup.exists()) {
            check(backup.renameTo(rpfKeyDirectory)) { "Cannot recover the previous key set" }
        }
        staging.deleteRecursively()
        check(staging.mkdirs()) { "Cannot create temporary key import folder" }
        var committed = false
        try {
            val importedSizes = linkedMapOf<String, Long>()
            for ((name, expectedBytes) in RpfKeyMaterialPolicy.expectedFileSizes) {
                val document = source.findFile(name) ?: continue
                check(document.isFile) { name + " is not a file" }
                val target = java.io.File(staging, name)
                copyUriToFile(document.uri, target, expectedBytes)
                importedSizes[name] = target.length()
            }
            RpfKeyMaterialPolicy.validate(importedSizes)
            val stagedReport = NativeAssets.inspectRpfKeys(staging.absolutePath)
            val hasNgSet = importedSizes.containsKey("gtav_ng_key.dat")
            check(stagedReport.contains("AES-256: ready") == importedSizes.containsKey("gtav_aes_key.dat")) {
                "RAGEKIT did not accept the imported AES key file"
            }
            check(stagedReport.contains("NG key material: ready") == hasNgSet) {
                "RAGEKIT did not accept the complete imported NG key set"
            }
            check(stagedReport.contains("AWC audio key: ready") == importedSizes.containsKey("gtav_awc_key.dat")) {
                "RAGEKIT did not accept the imported AWC key file"
            }

            // Preserve the current set until every new file has passed validation.
            if (backup.exists()) check(backup.deleteRecursively()) { "Cannot clear stale key backup" }
            if (rpfKeyDirectory.exists()) {
                check(rpfKeyDirectory.renameTo(backup)) {
                    "Cannot preserve the previous key set; import was not applied"
                }
            }
            if (!staging.renameTo(rpfKeyDirectory)) {
                if (backup.exists() && !rpfKeyDirectory.exists()) backup.renameTo(rpfKeyDirectory)
                error("Cannot activate the new key set")
            }
            committed = true
            backup.deleteRecursively()
            return stagedReport + "\nKey import validated and committed; the previous set was retained until activation."
        } finally {
            if (!committed) {
                staging.deleteRecursively()
                if (backup.exists() && !rpfKeyDirectory.exists()) backup.renameTo(rpfKeyDirectory)
            }
        }
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
                    if (total > maxBytes) error("Selected file exceeds the " + (maxBytes / (1024 * 1024)) + " MiB copy limit")
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

    private fun writeOutputFileSafely(
        source: java.io.File,
        directory: DocumentFile,
        requestedName: String
    ): DocumentFile {
        check(requestedName.isNotBlank() && requestedName != "." && requestedName != ".." &&
            !requestedName.contains('/') && !requestedName.contains('\\') && !requestedName.contains(':')) {
            "Unsafe RPF filename cannot be preserved: " + requestedName
        }
        check(directory.findFile(requestedName) == null) {
            "Output already contains " + requestedName + ". Choose an empty output folder to preserve DLC filenames and avoid overwriting."
        }
        val partialName = requestedName + ".partial"
        check(directory.findFile(partialName) == null) {
            "A partial output already exists: " + partialName + ". Remove it before retrying."
        }
        val partial = directory.createFile("application/octet-stream", partialName)
            ?: error("Cannot create temporary output file " + partialName)
        var finalized: DocumentFile? = null
        try {
            var copied = 0L
            val expectedDigest = MessageDigest.getInstance("SHA-256")
            contentResolver.openOutputStream(partial.uri, "w")?.use { out ->
                source.inputStream().use { input ->
                    val buffer = ByteArray(64 * 1024)
                    while (true) {
                        val count = input.read(buffer)
                        if (count < 0) break
                        copied += count
                        expectedDigest.update(buffer, 0, count)
                        out.write(buffer, 0, count)
                    }
                    out.flush()
                }
            } ?: error("Cannot write temporary Android RPF output")
            check(copied == source.length()) {
                "Output copy length mismatch: expected " + source.length() + " bytes, copied " + copied
            }
            val storedLength = partial.length()
            check(storedLength == 0L || storedLength == copied) {
                "Storage provider reported an incomplete temporary RPF output"
            }
            check(partial.renameTo(requestedName)) {
                "Storage provider could not finalize the RPF output filename; temporary output was discarded: " + partialName
            }
            val result = directory.findFile(requestedName)
                ?: error("Final RPF output could not be found after rename")
            finalized = result
            val finalLength = result.length()
            check(finalLength == 0L || finalLength == copied) {
                "Final RPF output size does not match the completed archive"
            }

            // Some SAF providers report length=0 even for non-empty files. Read the
            // finalized document back and compare a streaming SHA-256 instead of
            // trusting metadata alone. This uses a fixed 64 KiB buffer.
            val actualDigest = MessageDigest.getInstance("SHA-256")
            var verifiedBytes = 0L
            contentResolver.openInputStream(result.uri)?.use { input ->
                val buffer = ByteArray(64 * 1024)
                while (true) {
                    val count = input.read(buffer)
                    if (count < 0) break
                    verifiedBytes += count
                    actualDigest.update(buffer, 0, count)
                }
            } ?: error("Cannot read back the finalized Android RPF output")
            check(verifiedBytes == copied) {
                "Final RPF read-back length mismatch: expected " + copied + " bytes, got " + verifiedBytes
            }
            check(MessageDigest.isEqual(expectedDigest.digest(), actualDigest.digest())) {
                "Final RPF read-back SHA-256 does not match the generated archive"
            }
            return result
        } catch (error: Throwable) {
            (finalized ?: partial).delete()
            throw error
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
            appendLine("GTA V RPF INVENTORY — ANDROID CONVERTER 0.3.7")
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
            appendLine("Bulk conversion supports OPEN and AES/NG encrypted RPF7 archives when matching user-provided keys are imported. Supported Gen9 YTD/YDR/YDD/YFT/YPT resources are converted to Legacy before RPF7 repacking; other formats remain unchanged.")
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
        private const val MAX_NAMES_BYTES = 64L * 1024L * 1024L
        private const val MAX_TOC_BYTES = 80L * 1024L * 1024L
        private const val MAX_BULK_ARCHIVES = 500
        private const val MAX_FOLDER_ITEMS = 250_000
        private const val MAX_ARCHIVE_BYTES = 4L * 1024L * 1024L * 1024L - 1L
        private const val MAX_DLC_AUDIT_PACKS = 512
        private const val MAX_DLC_AUDIT_TOTAL_BYTES = 16L * 1024L * 1024L * 1024L
        private val ASSET_EXTENSIONS = setOf("ytd", "ydr", "ydd", "yft", "ybn", "ymap", "ytyp", "ycd", "ysc", "awc", "rel", "dat")
    }
}

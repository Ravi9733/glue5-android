package com.ravi9733.rpfexplorer

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.view.Gravity
import android.widget.*
import java.io.*
import java.util.zip.Inflater
import java.util.zip.InflaterInputStream
import kotlin.concurrent.thread

class MainActivity : Activity() {
    private lateinit var status: TextView
    private lateinit var list: LinearLayout
    private lateinit var openButton: Button
    private lateinit var search: EditText
    private var archive: RpfArchive? = null
    private var currentDir = 0
    private val PICK_RPF = 10
    private val SAVE_FILE = 11
    private var pendingExtract: RpfEntry? = null
    private var pendingYscDecrypt = false
    private var editor: RpfEditSession? = null
    private var ngMaterial: GtaNgMaterial? = null
    private var pendingEditEntry: RpfEntry? = null
    private var editAction: String? = null
    private val PICK_EDIT_SOURCE = 12
    private val SAVE_RPF = 13
    private val IMPORT_GTA5_EXE = 14
    private lateinit var addButton: Button
    private lateinit var keyButton: Button
    private lateinit var saveButton: Button
    private lateinit var backButton: Button
    private data class NestedFrame(val archive: RpfArchive, val editor: RpfEditSession, val directory: Int, val parentEntryIndex: Int)
    private val nestedStack = mutableListOf<NestedFrame>()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val root = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(18,18,18,18); setBackgroundColor(0xFF101318.toInt()) }
        val title = TextView(this).apply { text = "GTA V  •  RPF EXPLORER"; textSize = 22f; setTextColor(0xFFFFFFFF.toInt()); gravity = Gravity.CENTER_VERTICAL; setPadding(0,8,0,14) }
        status = TextView(this).apply { text = "Open a GTA V PC .rpf archive"; textSize = 13f; setTextColor(0xFFB8C3D1.toInt()); setPadding(0,0,0,12) }
        ngMaterial = runCatching { GtaNgMaterial.load(File(filesDir, "gtav-ng-material.bin")) }.getOrNull()
        openButton = Button(this).apply { text = "OPEN RPF FILE"; setOnClickListener { openArchive() } }
        addButton = Button(this).apply { text = "ADD FILE TO THIS FOLDER"; isEnabled = false; setOnClickListener { chooseEditSource("add", null) } }
        keyButton = Button(this).apply {
            text = when {
                ngMaterial != null && File(filesDir, "gtav-aes-key.bin").exists() -> "GTA V KEYS READY (AES + NG)"
                File(filesDir, "gtav-aes-key.bin").exists() -> "AES KEY READY • IMPORT GTA5.EXE FOR NG"
                else -> "IMPORT GTA5.EXE (AES + NG)"
            }
            setOnClickListener { importGtaExecutable() }
            setOnLongClickListener {
                android.app.AlertDialog.Builder(this@MainActivity)
                    .setMessage("Delete the locally stored GTA V AES key and NG key material from this app?")
                    .setPositiveButton("Delete key") { _, _ ->
                        File(filesDir, "gtav-aes-key.bin").delete()
                        File(filesDir, "gtav-ng-material.bin").delete()
                        ngMaterial = null
                        text = "IMPORT GTA5.EXE (AES + NG)"
                        Toast.makeText(this@MainActivity, "Local AES + NG key material deleted", Toast.LENGTH_SHORT).show()
                    }.setNegativeButton("Cancel", null).show()
                true
            }
        }
        saveButton = Button(this).apply { text = "SAVE AS NEW RPF"; isEnabled = false; setOnClickListener { saveEditedArchive() } }
        search = EditText(this).apply { hint = "Search current folder"; setSingleLine(true); setTextColor(0xFFFFFFFF.toInt()); setHintTextColor(0xFF8B96A5.toInt()); visibility = android.view.View.GONE }
        backButton = Button(this).apply { text = "↑  PARENT FOLDER"; setOnClickListener {
            if (currentDir != 0) { currentDir = archive?.parentOf(currentDir) ?: 0; render() }
            else if (nestedStack.isNotEmpty()) finishNested(false)
        } }
        list = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        val scroll = ScrollView(this).apply { addView(list) }
        root.addView(title); root.addView(status); root.addView(openButton); root.addView(keyButton); root.addView(addButton); root.addView(saveButton); root.addView(backButton); root.addView(search); root.addView(scroll, LinearLayout.LayoutParams(-1,0,1f))
        setContentView(root)
        search.addTextChangedListener(SimpleWatcher { render() })
    }

    private fun openArchive() {
        val i = Intent(Intent.ACTION_OPEN_DOCUMENT).apply { addCategory(Intent.CATEGORY_OPENABLE); type = "*/*"; putExtra(Intent.EXTRA_MIME_TYPES, arrayOf("application/octet-stream")); addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION) }
        startActivityForResult(i, PICK_RPF)
    }

    @Deprecated("Deprecated in Android API; retained for broad device compatibility")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (resultCode != Activity.RESULT_OK || data?.data == null) return
        val uri = data.data!!
        if (requestCode == PICK_RPF) {
            val archiveDisplayName = contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                if (cursor.moveToFirst()) cursor.getString(0) else null
            } ?: "archive.rpf"
            status.text = "Reading archive…"
            val temp = File(cacheDir, "selected-${System.currentTimeMillis()}.rpf")
            thread {
                try {
                    contentResolver.openInputStream(uri).use { input -> FileOutputStream(temp).use { out -> input!!.copyTo(out, 1024 * 1024) } }
                    val keyFile = File(filesDir, "gtav-aes-key.bin")
                    val parsed = RpfArchive.open(temp, if (keyFile.exists()) keyFile.readBytes() else null, ngMaterial, archiveDisplayName, deleteBackingOnClose = true)
                    runOnUiThread {
                        archive?.close()
                        editor?.dispose()
                        nestedStack.forEach { it.archive.close(); it.editor.dispose() }
                        nestedStack.clear()
                        archive = parsed; currentDir = 0
                        editor = RpfEditSession(parsed, File(cacheDir, "rpf-edit-${System.currentTimeMillis()}").apply { mkdirs() })
                        updateNavigationButtons()
                        addButton.isEnabled = true; saveButton.isEnabled = true
                        status.text = "${parsed.entries.size} entries • ${parsed.encryptionLabel} • ${temp.length()/1024/1024} MiB"
                        search.visibility = android.view.View.VISIBLE
                        render()
                    }
                } catch (e: Exception) {
                    temp.delete()
                    runOnUiThread { status.text = "Could not open: ${e.message}"; Toast.makeText(this, e.message ?: "Invalid archive", Toast.LENGTH_LONG).show() }
                }
            }
        } else if (requestCode == IMPORT_GTA5_EXE) {
            status.text = "Scanning GTA5.exe for AES + NG key material…"
            val exe = File(cacheDir, "gta5-exe-${System.currentTimeMillis()}.bin")
            thread {
                try {
                    contentResolver.openInputStream(uri).use { input -> FileOutputStream(exe).use { output -> input!!.copyTo(output, 1024 * 1024) } }
                    val key = RpfCrypto.extractAesKey(exe) { done, total ->
                        runOnUiThread { status.text = "Scanning AES key… ${(done * 100 / total).toInt()}%" }
                    }
                    File(filesDir, "gtav-aes-key.bin").writeBytes(key)
                    val ngError = runCatching {
                        GtaNgMaterial.extractFromExe(exe) { stage, percent ->
                            runOnUiThread { status.text = "$stage… $percent%" }
                        }.also { it.save(File(filesDir, "gtav-ng-material.bin")) }
                    }.exceptionOrNull()
                    ngMaterial = if (ngError == null) GtaNgMaterial.load(File(filesDir, "gtav-ng-material.bin")) else null
                    exe.delete()
                    runOnUiThread {
                        keyButton.text = if (ngMaterial != null) "GTA V KEYS READY (AES + NG)" else "AES KEY READY • NG UNSUPPORTED"
                        status.text = if (ngMaterial != null) "AES + NG material extracted from your GTA5.exe and stored privately on this device."
                            else "AES key ready. NG extraction failed: ${ngError?.message ?: "unknown error"}"
                        Toast.makeText(this, if (ngMaterial != null) "AES + NG keys ready" else "AES ready; NG not available for this GTA5.exe build", Toast.LENGTH_LONG).show()
                    }
                } catch (e: Exception) {
                    exe.delete()
                    runOnUiThread { status.text = "AES key extraction failed: ${e.message}"; Toast.makeText(this, e.message ?: "Key scan failed", Toast.LENGTH_LONG).show() }
                }
            }
        } else if (requestCode == PICK_EDIT_SOURCE) {
            val action = editAction ?: return
            val originalName = contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                if (cursor.moveToFirst()) cursor.getString(0) else null
            } ?: "imported-file.bin"
            val target = File(cacheDir, "edit-source-${System.currentTimeMillis()}")
            thread {
                try {
                    contentResolver.openInputStream(uri).use { input -> FileOutputStream(target).use { output -> input!!.copyTo(output, 1024 * 1024) } }
                    runOnUiThread {
                        try {
                            if (action == "add") editor!!.add(currentDir, target, originalName)
                            else editor!!.replace(pendingEditEntry!!.index, target)
                            target.delete()
                            status.text = "Edits staged: ${editor!!.stagedCount()} • Original archive unchanged"
                            render()
                        } catch (e: Exception) { Toast.makeText(this, e.message ?: "Edit failed", Toast.LENGTH_LONG).show() }
                    }
                } catch (e: Exception) {
                    target.delete()
                    runOnUiThread { Toast.makeText(this, "Import failed: ${e.message}", Toast.LENGTH_LONG).show() }
                }
            }
        } else if (requestCode == SAVE_RPF) {
            val destination = File(cacheDir, "edited-${System.currentTimeMillis()}.rpf")
            thread {
                try {
                    editor!!.saveAs(destination)
                    contentResolver.openOutputStream(uri)?.use { output -> destination.inputStream().use { it.copyTo(output, 1024 * 1024) } } ?: error("Cannot create output document")
                    destination.delete()
                    runOnUiThread { status.text = "Saved new RPF • source archive untouched"; Toast.makeText(this, "New RPF saved successfully", Toast.LENGTH_LONG).show() }
                } catch (e: Exception) {
                    runOnUiThread { status.text = "Save blocked: ${e.message}"; Toast.makeText(this, e.message ?: "Save failed", Toast.LENGTH_LONG).show() }
                } finally {
                    destination.delete()
                }
            }
        } else if (requestCode == SAVE_FILE) {
            val entry = pendingExtract ?: return
            val decryptYsc = pendingYscDecrypt
            pendingYscDecrypt = false
            thread {
                try {
                    contentResolver.openOutputStream(uri)?.use { output ->
                        val staged = editor?.replacementFile(entry.index)
                        if (decryptYsc) archive!!.extractEncryptedYsc(entry, output)
                        else if (staged != null) staged.inputStream().use { it.copyTo(output) }
                        else if (entry.isResource) archive!!.copyRaw(entry, output)
                        else archive!!.extract(entry, output)
                    } ?: error("Cannot open destination")
                    runOnUiThread { Toast.makeText(this, if (decryptYsc) "Decrypted YSC exported" else "Extracted ${entry.name}", Toast.LENGTH_LONG).show() }
                } catch (e: Exception) { runOnUiThread { Toast.makeText(this, "Extract failed: ${e.message}", Toast.LENGTH_LONG).show() } }
            }
        }
    }

    private fun importGtaExecutable() {
        val i = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE); type = "*/*"
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        startActivityForResult(i, IMPORT_GTA5_EXE)
    }

    private fun chooseEditSource(action: String, entry: RpfEntry?) {
        editAction = action
        pendingEditEntry = entry
        val i = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE); type = "*/*"
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        startActivityForResult(i, PICK_EDIT_SOURCE)
    }

    private fun saveEditedArchive() {
        val count = editor?.stagedCount() ?: 0
        if (nestedStack.isNotEmpty()) {
            android.app.AlertDialog.Builder(this)
                .setTitle("Apply nested archive")
                .setMessage("$count staged changes will be packed into the parent RPF. The original archive file will remain unchanged.")
                .setPositiveButton("Apply to parent") { _, _ -> applyNestedToParent() }
                .setNegativeButton("Cancel", null).show()
            return
        }
        android.app.AlertDialog.Builder(this)
            .setTitle("Save as a new RPF")
            .setMessage("$count staged changes will be written to a separate output file. The selected source archive will not be modified.")
            .setPositiveButton("Choose output") { _, _ ->
                val i = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE); type = "application/octet-stream"
                    putExtra(Intent.EXTRA_TITLE, "edited-gta-v.rpf")
                }
                startActivityForResult(i, SAVE_RPF)
            }
            .setNegativeButton("Cancel", null).show()
    }

    private fun rscPageBytes(flags: Long): Long {
        val count =
            (((flags ushr 27) and 1L) shl 0) +
            (((flags ushr 26) and 1L) shl 1) +
            (((flags ushr 25) and 1L) shl 2) +
            (((flags ushr 24) and 1L) shl 3) +
            (((flags ushr 17) and 0x7FL) shl 4) +
            (((flags ushr 11) and 0x3FL) shl 5) +
            (((flags ushr 7) and 0xFL) shl 6) +
            (((flags ushr 5) and 0x3L) shl 7) +
            (((flags ushr 4) and 1L) shl 8)
        return (0x200L shl (flags and 0xFL).toInt()) * count
    }

    private fun showResourceInspector(entry: RpfEntry) {
        try {
            val stagedResource = editor?.replacementFile(entry.index)
            val bytes = if (stagedResource != null) stagedResource.inputStream().use { input ->
                val buffer = ByteArray(32)
                val count = input.read(buffer)
                if (count < 0) ByteArray(0) else buffer.copyOf(count)
            } else archive!!.readRawPrefix(entry, 32)
            val hex = bytes.joinToString(" ") { "%02X".format(it) }
            val magic = if (bytes.size >= 4) String(bytes.copyOfRange(0, 4), Charsets.US_ASCII) else "Unknown"
            val version = if (bytes.size >= 8) {
                (bytes[4].toLong() and 255L) or ((bytes[5].toLong() and 255L) shl 8) or
                    ((bytes[6].toLong() and 255L) shl 16) or ((bytes[7].toLong() and 255L) shl 24)
            } else 0L
            val info = buildString {
                appendLine("Name: ${entry.name}")
                appendLine("RSC7 header: ${if (magic == "RSC7") "valid magic" else "unrecognized ($magic)"}")
                appendLine("Encrypted script resource: ${if (entry.encrypted) "yes (.ysc)" else "no"}")
                appendLine("RSC7 version: 0x${version.toString(16).uppercase()}")
                appendLine("System pages: ${rscPageBytes(entry.systemFlags)} bytes")
                appendLine("Graphics pages: ${rscPageBytes(entry.graphicsFlags)} bytes")
                appendLine("Payload: ${stagedResource?.length() ?: entry.storedSize} bytes${if (stagedResource != null) " (staged replacement)" else ""}")
                appendLine("System page flags: 0x${entry.systemFlags.toString(16).uppercase()}")
                appendLine("Graphics page flags: 0x${entry.graphicsFlags.toString(16).uppercase()}")
                appendLine("First 32 bytes:")
                append(hex.ifBlank { "(empty)" })
                appendLine()
                appendLine()
                if (entry.encrypted) {
                    appendLine("WARNING: YSC script payload is encrypted. Export decrypted YSC decrypts and decompresses the script for inspection; encrypted YSC repacking remains blocked.")
                }
                append("This is a raw RSC7 header inspector. It does not decode texture/model data or generate a visual preview.")
            }
            android.app.AlertDialog.Builder(this)
                .setTitle("RSC7 Resource Inspector")
                .setMessage(info)
                .setPositiveButton("Export raw RSC7") { _, _ ->
                    pendingExtract = entry
                    pendingYscDecrypt = false
                    val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
                        addCategory(Intent.CATEGORY_OPENABLE)
                        type = "application/octet-stream"
                        putExtra(Intent.EXTRA_TITLE, entry.name.substringAfterLast('/'))
                    }
                    startActivityForResult(intent, SAVE_FILE)
                }
                .setNeutralButton(if (entry.encrypted && archive!!.canDecryptYsc(entry)) "Export decrypted YSC" else null) { _, _ ->
                    pendingExtract = entry
                    pendingYscDecrypt = true
                    val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
                        addCategory(Intent.CATEGORY_OPENABLE)
                        type = "application/octet-stream"
                        putExtra(Intent.EXTRA_TITLE, entry.name.substringAfterLast('/') + ".decrypted")
                    }
                    startActivityForResult(intent, SAVE_FILE)
                }
                .setNegativeButton("Close", null)
                .show()
        } catch (e: Exception) {
            Toast.makeText(this, "Resource inspection failed: ${e.message}", Toast.LENGTH_LONG).show()
        }
    }

    private fun render() {
        list.removeAllViews()
        val a = archive ?: return
        val query = search.text.toString().trim().lowercase()
        a.children(currentDir).filter { editor?.isRemoved(it.index) != true }.filter { query.isEmpty() || it.name.lowercase().contains(query) }.forEach { entry ->
            val row = TextView(this).apply {
                text = (if (entry.isDirectory) "📁  " else if (entry.isResource) "◈  " else "📄  ") + entry.name + (if (!entry.isDirectory) "   (${if (entry.isResource) entry.storedSize else entry.uncompressedSize} B)" else "") + (if (editor?.isReplaced(entry.index) == true) "  • REPLACED" else "")
                textSize = 15f; setTextColor(0xFFE5EAF1.toInt()); setPadding(12,16,8,16)
                setBackgroundColor(0xFF1A2029.toInt())
                setOnClickListener {
                    if (entry.isDirectory) { currentDir = entry.index; render() }
                    else if (entry.name.endsWith(".rpf", ignoreCase = true) && !entry.isResource) openNested(entry)
                    else if (entry.isResource) showResourceInspector(entry)
                    else if (entry.encrypted) Toast.makeText(this@MainActivity, "This file is encrypted. GTA V keys are required.", Toast.LENGTH_LONG).show()
                    else { pendingExtract = entry; val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply { addCategory(Intent.CATEGORY_OPENABLE); type = "application/octet-stream"; putExtra(Intent.EXTRA_TITLE, entry.name.substringAfterLast('/')) }; startActivityForResult(intent, SAVE_FILE) }
                }
                setOnLongClickListener {
                    if (entry.isDirectory) {
                        android.app.AlertDialog.Builder(this@MainActivity)
                            .setMessage("Remove this folder and all its contents from the new archive?")
                            .setPositiveButton("Remove") { _, _ ->
                                try { editor!!.remove(entry.index); status.text = "Edits staged: ${editor!!.stagedCount()} • Original unchanged"; render() }
                                catch (e: Exception) { Toast.makeText(this@MainActivity, e.message ?: "Remove failed", Toast.LENGTH_LONG).show() }
                            }.setNegativeButton("Cancel", null).show()
                    } else {
                        val options = arrayOf("Replace file", "Remove file")
                        android.app.AlertDialog.Builder(this@MainActivity).setTitle(entry.name).setItems(options) { _, which ->
                            if (which == 0) chooseEditSource("replace", entry)
                            else android.app.AlertDialog.Builder(this@MainActivity)
                                .setMessage("Remove this file from the new archive? The original remains unchanged.")
                                .setPositiveButton("Remove") { _, _ ->
                                    try { editor!!.remove(entry.index); status.text = "Edits staged: ${editor!!.stagedCount()} • Original unchanged"; render() }
                                    catch (e: Exception) { Toast.makeText(this@MainActivity, e.message ?: "Remove failed", Toast.LENGTH_LONG).show() }
                                }.setNegativeButton("Cancel", null).show()
                        }.show()
                    }
                    true
                }
            }
            val lp = LinearLayout.LayoutParams(-1,-2); lp.bottomMargin = 3; list.addView(row,lp)
        }
        editor?.addedNames(currentDir)?.forEach { addedName ->
            list.addView(TextView(this).apply {
                text = "＋  $addedName  • NEW"
                textSize = 15f; setTextColor(0xFF8FE3B0.toInt()); setPadding(12,16,8,16)
                setBackgroundColor(0xFF1A2029.toInt())
                setOnLongClickListener {
                    android.app.AlertDialog.Builder(this@MainActivity)
                        .setMessage("Remove this newly added file from the staged archive?")
                        .setPositiveButton("Remove") { _, _ ->
                            editor?.removeAdded(currentDir, addedName)
                            status.text = "Edits staged: ${editor?.stagedCount() ?: 0} • Original unchanged"
                            render()
                        }.setNegativeButton("Cancel", null).show()
                    true
                }
            }, LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = 3 })
        }
        if (list.childCount == 0) list.addView(TextView(this).apply { text="No matching entries"; setTextColor(0xFFB8C3D1.toInt()); setPadding(12,24,12,24) })
        updateNavigationButtons()
    }

    private fun updateNavigationButtons() {
        if (!::backButton.isInitialized || !::saveButton.isInitialized) return
        backButton.text = if (currentDir != 0) "↑  PARENT FOLDER" else if (nestedStack.isNotEmpty()) "←  EXIT NESTED RPF" else "↑  PARENT FOLDER"
        saveButton.text = if (nestedStack.isNotEmpty()) "APPLY NESTED CHANGES TO PARENT" else "SAVE AS NEW RPF"
    }

    private fun openNested(entry: RpfEntry) {
        status.text = "Opening nested archive: ${entry.name}…"
        val nestedFile = File(cacheDir, "nested-${System.currentTimeMillis()}.rpf")
        thread {
            try {
                val stagedNested = editor?.replacementFile(entry.index)
                if (stagedNested != null) stagedNested.copyTo(nestedFile, overwrite = true)
                else FileOutputStream(nestedFile).use { output -> archive!!.extract(entry, output) }
                val nestedArchive = RpfArchive.open(nestedFile, archive?.aesKeyCopy(), ngMaterial, entry.name, deleteBackingOnClose = true)
                runOnUiThread {
                    nestedStack += NestedFrame(archive!!, editor!!, currentDir, entry.index)
                    archive = nestedArchive
                    currentDir = 0
                    editor = RpfEditSession(nestedArchive, File(cacheDir, "nested-edit-${System.currentTimeMillis()}").apply { mkdirs() })
                    status.text = "Nested RPF • ${nestedArchive.entries.size} entries • ${nestedArchive.encryptionLabel}"
                    render()
                }
            } catch (e: Exception) {
                nestedFile.delete()
                runOnUiThread { status.text = "Nested RPF open failed: ${e.message}"; Toast.makeText(this, e.message ?: "Nested archive error", Toast.LENGTH_LONG).show() }
            }
        }
    }

    private fun applyNestedToParent() {
        val frame = nestedStack.lastOrNull() ?: return
        val nestedOutput = File(cacheDir, "nested-edited-${System.currentTimeMillis()}.rpf")
        thread {
            try {
                editor!!.saveAs(nestedOutput)
                frame.editor.replace(frame.parentEntryIndex, nestedOutput)
                nestedOutput.delete()
                runOnUiThread {
                    archive?.close()
                    editor?.dispose()
                    nestedStack.removeAt(nestedStack.lastIndex)
                    archive = frame.archive; editor = frame.editor; currentDir = frame.directory
                    status.text = "Nested archive changes staged in parent • original files untouched"
                    render()
                }
            } catch (e: Exception) {
                runOnUiThread { status.text = "Nested save blocked: ${e.message}"; Toast.makeText(this, e.message ?: "Nested save failed", Toast.LENGTH_LONG).show() }
            }
        }
    }

    override fun onDestroy() {
        try { archive?.close() } catch (_: Exception) {}
        try { editor?.dispose() } catch (_: Exception) {}
        nestedStack.forEach {
            try { it.archive.close() } catch (_: Exception) {}
            try { it.editor.dispose() } catch (_: Exception) {}
        }
        super.onDestroy()
    }

    private fun finishNested(save: Boolean) {
        if (save) { applyNestedToParent(); return }
        android.app.AlertDialog.Builder(this)
            .setMessage("Discard staged changes in this nested archive and return to the parent?")
            .setPositiveButton("Discard") { _, _ ->
                archive?.close()
                editor?.dispose()
                val frame = nestedStack.removeAt(nestedStack.lastIndex)
                archive = frame.archive; editor = frame.editor; currentDir = frame.directory
                status.text = "Returned to parent archive • nested edits discarded"
                render()
            }.setNegativeButton("Cancel", null).show()
    }
}

private class SimpleWatcher(val changed: () -> Unit) : android.text.TextWatcher {
    override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
    override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) { changed() }
    override fun afterTextChanged(s: android.text.Editable?) {}
}

internal data class RpfEntry(
    val index: Int, val name: String, val parent: Int, val isDirectory: Boolean, val isResource: Boolean,
    val offset: Long = 0, val storedSize: Long = 0, val uncompressedSize: Long = 0,
    val encrypted: Boolean = false, val childrenStart: Int = 0, val childrenCount: Int = 0, val systemFlags: Long = 0, val graphicsFlags: Long = 0
)

internal class RpfArchive private constructor(
    private val file: RandomAccessFile,
    private val filePath: File,
    val entries: List<RpfEntry>,
    val encryptionLabel: String,
    private val aesKey: ByteArray?,
    private val ngMaterial: GtaNgMaterial?,
    val archiveName: String,
    private val deleteBackingOnClose: Boolean
) {
    fun children(parent: Int) = entries.filter { it.parent == parent && it.index != parent }
    fun parentOf(index: Int) = entries.firstOrNull { it.index == index }?.parent ?: 0
    fun close() {
        file.close()
        if (deleteBackingOnClose) filePath.delete()
    }
    fun aesKeyCopy(): ByteArray? = aesKey?.clone()

    fun readRawPrefix(e: RpfEntry, maxBytes: Int): ByteArray {
        require(!e.isDirectory && maxBytes >= 0)
        val size = minOf(e.storedSize, maxBytes.toLong()).toInt()
        val result = ByteArray(size)
        synchronized(file) {
            file.seek(e.offset)
            file.readFully(result)
        }
        return result
    }

    fun copyRaw(e: RpfEntry, output: OutputStream) {
        require(!e.isDirectory) { "Cannot copy a directory payload" }
        val size = e.storedSize
        require(size >= 0 && e.offset >= 0 && e.offset + size <= file.length()) { "Invalid entry data range" }
        synchronized(file) {
            file.seek(e.offset)
            val buffer = ByteArray(64 * 1024)
            var remaining = size
            while (remaining > 0) {
                val n = file.read(buffer, 0, minOf(buffer.size.toLong(), remaining).toInt())
                if (n < 0) error("Unexpected end of archive")
                output.write(buffer, 0, n); remaining -= n
            }
        }
    }

    fun canDecryptYsc(e: RpfEntry): Boolean =
        e.isResource && e.encrypted && e.name.endsWith(".ysc", ignoreCase = true) &&
            ((encryptionLabel == "AES" && aesKey != null) || (encryptionLabel == "NG" && ngMaterial != null))

    /**
     * CodeWalker-compatible YSC export: skip the 16-byte RSC7 resource header,
     * decrypt the encrypted compressed payload using the script name and full
     * resource size for NG, then inflate raw DEFLATE into the standalone script.
     * The archive itself is never modified and encrypted YSC repacking stays blocked.
     */
    fun extractEncryptedYsc(e: RpfEntry, output: OutputStream) {
        require(canDecryptYsc(e)) { "A compatible local AES/NG key bundle is required to decrypt this YSC resource" }
        require(e.storedSize > 16L) { "Encrypted YSC resource is truncated" }
        val encryptedSize = e.storedSize - 16L
        require(e.offset >= 0L && e.offset + e.storedSize <= file.length()) { "YSC resource data range is invalid" }
        val temp = File.createTempFile("rpf-ysc-dec-", ".tmp", filePath.parentFile)
        try {
            synchronized(file) {
                file.seek(e.offset + 16L)
                val bounded = object : InputStream() {
                    private var remaining = encryptedSize
                    override fun read(): Int {
                        if (remaining <= 0L) return -1
                        val value = file.read()
                        if (value >= 0) remaining--
                        return value
                    }
                    override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
                        if (remaining <= 0L) return -1
                        val n = file.read(buffer, offset, minOf(length.toLong(), remaining).toInt())
                        if (n > 0) remaining -= n
                        return n
                    }
                }
                FileOutputStream(temp).use { decoded ->
                    when (encryptionLabel) {
                        "AES" -> RpfCrypto.decryptAes(bounded, encryptedSize, decoded, aesKey!!)
                        "NG" -> ngMaterial!!.decrypt(bounded, encryptedSize, decoded, e.name, e.storedSize)
                        else -> error("YSC decryption is not available for this archive encryption mode")
                    }
                }
            }
            temp.inputStream().buffered().use { compressed ->
                InflaterInputStream(compressed, Inflater(true)).use { it.copyTo(output, 64 * 1024) }
            }
        } finally {
            temp.delete()
        }
    }

    fun extract(e: RpfEntry, output: OutputStream) {
        require(!e.isDirectory && !e.isResource) { "Resource extraction requires RSC7 decoding" }
        if (e.encrypted) require(
            (encryptionLabel == "AES" && aesKey != null) || (encryptionLabel == "NG" && ngMaterial != null)
        ) { "Encrypted binary payload needs compatible local GTA V AES/NG material" }
        val size = if (e.storedSize == 0L) e.uncompressedSize else e.storedSize
        require(size >= 0L) { "Invalid entry size" }
        if (e.storedSize > 0L) require(size <= Int.MAX_VALUE.toLong()) { "Compressed entry is too large for this extractor" }
        synchronized(file) {
            file.seek(e.offset)
            if (e.storedSize == 0L && !e.encrypted) {
                val bytes = ByteArray(64 * 1024)
                var remaining = size
                while (remaining > 0) {
                    val n = file.read(bytes, 0, minOf(bytes.size.toLong(), remaining).toInt())
                    if (n < 0) error("Unexpected end of archive")
                    output.write(bytes, 0, n); remaining -= n
                }
            } else if (e.storedSize == 0L && e.encrypted) {
                val limited = object : InputStream() {
                    private var remaining = size
                    override fun read(): Int {
                        if (remaining <= 0) return -1
                        val value = file.read()
                        if (value >= 0) remaining--
                        return value
                    }
                    override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
                        if (remaining <= 0) return -1
                        val n = file.read(buffer, offset, minOf(length.toLong(), remaining).toInt())
                        if (n > 0) remaining -= n
                        return n
                    }
                }
                if (encryptionLabel == "AES") RpfCrypto.decryptAes(limited, size, output, aesKey!!)
                else ngMaterial!!.decrypt(limited, size, output, e.name, e.uncompressedSize)
            } else {
                val stored = ByteArray(size.toInt())
                file.readFully(stored)
                val decoded = when {
                    !e.encrypted -> stored
                    encryptionLabel == "AES" -> RpfCrypto.decryptAes(stored, aesKey!!)
                    encryptionLabel == "NG" -> ngMaterial!!.decrypt(stored, e.name, e.uncompressedSize)
                    else -> error("Unsupported per-file encryption mode")
                }
                InflaterInputStream(ByteArrayInputStream(decoded), Inflater(true)).use { it.copyTo(output) }
            }
        }
    }

    companion object {
        fun open(
            f: File,
            aesKey: ByteArray? = null,
            ngMaterial: GtaNgMaterial? = null,
            archiveName: String = f.name,
            deleteBackingOnClose: Boolean = false
        ): RpfArchive {
            val raf = RandomAccessFile(f,"r")
            try {
                require(raf.length() >= 16) { "File is too small to be an RPF archive" }
                val magic = readU32(raf,0)
                require(magic == 0x52504637L) { "Not GTA V RPF7 (magic mismatch)" }
                val count = readU32(raf,4).toInt()
                val namesRaw = readU32(raf,8)
                val namesLen = (namesRaw and 0x0FFFFFFF).toInt()
                val enc = readU32(raf,12)
                require(count in 1..2_000_000) { "Invalid entry count" }
                val tocBytes = count.toLong()*16L
                val namesStart = 16L+tocBytes
                require(namesStart+namesLen <= raf.length()) { "Archive table is truncated" }
                val isAes = enc == 0x0FFFFFF9L
                val isNg = enc == 0x0FEFFFFFL
                require(enc == 0L || enc == 0x4E45504FL || isAes || isNg) { "Unknown RPF encryption marker: 0x${enc.toString(16)}" }
                if (isAes) require(aesKey?.size == 32) { "AES-encrypted RPF detected. Import a compatible GTA5.exe first to extract its AES key." }
                if (isNg) require(ngMaterial != null) { "NG-encrypted RPF detected. Import a compatible PC GTA5.exe to extract local NG key material." }
                val names = ByteArray(namesLen); raf.seek(namesStart); raf.readFully(names)
                if (isAes || isNg) {
                    val decodedNames = if (isAes) RpfCrypto.decryptAes(names, aesKey!!)
                        else ngMaterial!!.decrypt(names, archiveName, raf.length())
                    System.arraycopy(decodedNames, 0, names, 0, names.size)
                }
                val toc = ByteArray(tocBytes.toInt()); raf.seek(16L); raf.readFully(toc)
                val decodedToc = when {
                    isAes -> RpfCrypto.decryptAes(toc, aesKey!!)
                    isNg -> ngMaterial!!.decrypt(toc, archiveName, raf.length())
                    else -> toc
                }
                val raw = ArrayList<ByteArray>(count)
                for (i in 0 until count) raw.add(decodedToc.copyOfRange(i * 16, i * 16 + 16))
                fun u16(b:ByteArray,o:Int)= (b[o].toInt() and 255) or ((b[o+1].toInt() and 255) shl 8)
                fun u24(b:ByteArray,o:Int)= (b[o].toLong() and 255) or ((b[o+1].toLong() and 255) shl 8) or ((b[o+2].toLong() and 255) shl 16)
                fun u32(b:ByteArray,o:Int)= (b[o].toLong() and 255) or ((b[o+1].toLong() and 255) shl 8) or ((b[o+2].toLong() and 255) shl 16) or ((b[o+3].toLong() and 255) shl 24)
                fun name(off:Int):String {
                    require(off in names.indices) { "RPF entry name offset is outside the name table" }
                    var end = off
                    while (end < names.size && names[end].toInt() != 0) end++
                    require(end < names.size && end > off) { "RPF entry name is empty or not null-terminated" }
                    val value = String(names, off, end - off, Charsets.UTF_8)
                    require(value.none { it == '/' || it == '\\' || it == '\u0000' }) { "RPF entry contains an invalid path component" }
                    return value
                }
                data class Temp(val idx:Int,val nm:String,val dir:Boolean,val res:Boolean,val off:Long,val sz:Long,val usz:Long,val crypt:Boolean,val start:Int,val cnt:Int)
                val temps=ArrayList<Temp>(count)
                for(i in 0 until count) {
                    val b=raw[i]; val second=u32(b,4)
                    if(second==0x7FFFFF00L) {
                        val childStart = u32(b,8); val childCount = u32(b,12)
                        require(childStart <= count.toLong() && childCount <= count.toLong() && childStart + childCount <= count.toLong()) { "Directory child range is invalid" }
                        temps.add(Temp(i,name(u32(b,0).also { require(it <= Int.MAX_VALUE.toLong()) }.toInt()),true,false,0,0,0,false,childStart.toInt(),childCount.toInt()))
                    } else if((second and 0x80000000L)==0L) {
                        val off=u24(b,5)*512L; val sz=u24(b,2); val usz=u32(b,8); val cryptType=u32(b,12)
                        require(cryptType == 0L || cryptType == 1L) { "Unknown binary encryption type" }
                        val crypt=cryptType==1L
                        temps.add(Temp(i,name(u16(b,0)),false,false,off,sz,usz,crypt,0,0))
                    } else {
                        val resourceOffset = u24(b,5) and 0x7FFFFF
                        val encodedSize = u24(b,2)
                        val absolute = resourceOffset * 512L
                        val resourceName = name(u16(b,0))
                        val resourceSize = if (encodedSize == 0xFFFFFFL) {
                            require(absolute + 16L <= raf.length()) { "RSC7 resource header is truncated" }
                            val h = ByteArray(16); raf.seek(absolute); raf.readFully(h)
                            (h[7].toLong() and 255L) or ((h[14].toLong() and 255L) shl 8) or
                                ((h[5].toLong() and 255L) shl 16) or ((h[2].toLong() and 255L) shl 24)
                        } else encodedSize
                        require(resourceSize >= 16L && absolute + resourceSize <= raf.length()) { "RSC7 resource data range is invalid" }
                        val encryptedScript = resourceName.endsWith(".ysc", ignoreCase = true)
                        temps.add(Temp(i,resourceName,false,true,absolute,resourceSize,0,encryptedScript,0,0))
                    }
                }
                val parents=IntArray(count){0}
                val assigned=BooleanArray(count)
                assigned[0] = true
                fun assign(dir:Int, depth:Int) {
                    require(depth<256) { "Directory nesting is invalid" }
                    val t=temps[dir]; val from=t.start; val to=from+t.cnt
                    for (child in from until to) {
                        require(child != dir && !assigned[child]) { "RPF directory ranges overlap or contain a cycle" }
                        assigned[child] = true
                        parents[child] = dir
                        if (temps[child].dir) assign(child, depth + 1)
                    }
                }
                require(temps[0].dir) { "RPF root entry must be a directory" }
                assign(0,0)
                require(assigned.all { it }) { "RPF archive contains entries not referenced by the directory tree" }
                val result=temps.map { t ->
                    val rawEntry = raw[t.idx]
                    val sys = if (t.res) u32(rawEntry, 8) else 0L
                    val gfx = if (t.res) u32(rawEntry, 12) else 0L
                    RpfEntry(t.idx,t.nm,parents[t.idx],t.dir,t.res,t.off,t.sz,t.usz,t.crypt,t.start,t.cnt,sys,gfx)
                }
                val dataStart = ((namesStart + namesLen + 511L) / 512L) * 512L
                result.filter { !it.isDirectory }.forEach {
                    val payloadSize = if (it.isResource) it.storedSize else if (it.storedSize == 0L) it.uncompressedSize else it.storedSize
                    require(it.offset >= dataStart && payloadSize >= 0L && it.offset <= raf.length() - payloadSize) {
                        "Entry points outside the archive payload region"
                    }
                }
                val label = when (enc) { 0L -> "NONE"; 0x4E45504FL -> "OPEN"; 0x0FFFFFF9L -> "AES"; else -> "NG" }
                return RpfArchive(raf, f, result, label, aesKey?.clone(), ngMaterial, archiveName, deleteBackingOnClose)
            } catch(e:Exception) { raf.close(); throw e }
        }
        private fun readU32(r:RandomAccessFile,pos:Long):Long { r.seek(pos); return (r.readUnsignedByte().toLong()) or (r.readUnsignedByte().toLong() shl 8) or (r.readUnsignedByte().toLong() shl 16) or (r.readUnsignedByte().toLong() shl 24) }
    }
}

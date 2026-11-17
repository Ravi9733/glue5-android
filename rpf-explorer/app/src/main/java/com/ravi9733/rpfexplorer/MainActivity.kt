package com.ravi9733.rpfexplorer

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
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
    private var editor: RpfEditSession? = null
    private var pendingEditEntry: RpfEntry? = null
    private var editAction: String? = null
    private val PICK_EDIT_SOURCE = 12
    private val SAVE_RPF = 13
    private lateinit var addButton: Button
    private lateinit var saveButton: Button

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val root = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(18,18,18,18); setBackgroundColor(0xFF101318.toInt()) }
        val title = TextView(this).apply { text = "GTA V  •  RPF EXPLORER"; textSize = 22f; setTextColor(0xFFFFFFFF.toInt()); gravity = Gravity.CENTER_VERTICAL; setPadding(0,8,0,14) }
        status = TextView(this).apply { text = "Open a GTA V PC .rpf archive"; textSize = 13f; setTextColor(0xFFB8C3D1.toInt()); setPadding(0,0,0,12) }
        openButton = Button(this).apply { text = "OPEN RPF FILE"; setOnClickListener { openArchive() } }
        addButton = Button(this).apply { text = "ADD FILE TO THIS FOLDER"; isEnabled = false; setOnClickListener { chooseEditSource("add", null) } }
        saveButton = Button(this).apply { text = "SAVE AS NEW RPF"; isEnabled = false; setOnClickListener { saveEditedArchive() } }
        search = EditText(this).apply { hint = "Search current folder"; setSingleLine(true); setTextColor(0xFFFFFFFF.toInt()); setHintTextColor(0xFF8B96A5.toInt()); visibility = android.view.View.GONE }
        val back = Button(this).apply { text = "↑  PARENT FOLDER"; setOnClickListener { if (currentDir != 0) { currentDir = archive?.parentOf(currentDir) ?: 0; render() } } }
        list = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        val scroll = ScrollView(this).apply { addView(list) }
        root.addView(title); root.addView(status); root.addView(openButton); root.addView(addButton); root.addView(saveButton); root.addView(back); root.addView(search); root.addView(scroll, LinearLayout.LayoutParams(-1,0,1f))
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
            status.text = "Reading archive…"
            thread {
                try {
                    val temp = File(cacheDir, "selected-${System.currentTimeMillis()}.rpf")
                    contentResolver.openInputStream(uri).use { input -> FileOutputStream(temp).use { out -> input!!.copyTo(out, 1024 * 1024) } }
                    val parsed = RpfArchive.open(temp)
                    runOnUiThread {
                        archive = parsed; currentDir = 0
                        editor = RpfEditSession(parsed, File(cacheDir, "rpf-edit").apply { mkdirs() })
                        addButton.isEnabled = true; saveButton.isEnabled = true
                        status.text = "${parsed.entries.size} entries • ${parsed.encryptionLabel} • ${temp.length()/1024/1024} MiB"
                        search.visibility = android.view.View.VISIBLE
                        render()
                    }
                } catch (e: Exception) { runOnUiThread { status.text = "Could not open: ${e.message}"; Toast.makeText(this, e.message ?: "Invalid archive", Toast.LENGTH_LONG).show() } }
            }
        } else if (requestCode == PICK_EDIT_SOURCE) {
            val action = editAction ?: return
            val target = File(cacheDir, "edit-source-${System.currentTimeMillis()}")
            thread {
                try {
                    contentResolver.openInputStream(uri).use { input -> FileOutputStream(target).use { output -> input!!.copyTo(output, 1024 * 1024) } }
                    runOnUiThread {
                        try {
                            if (action == "add") editor!!.add(currentDir, target)
                            else editor!!.replace(pendingEditEntry!!.index, target)
                            status.text = "Edits staged: ${editor!!.stagedCount()} • Original archive unchanged"
                            render()
                        } catch (e: Exception) { Toast.makeText(this, e.message ?: "Edit failed", Toast.LENGTH_LONG).show() }
                    }
                } catch (e: Exception) { runOnUiThread { Toast.makeText(this, "Import failed: ${e.message}", Toast.LENGTH_LONG).show() } }
            }
        } else if (requestCode == SAVE_RPF) {
            val destination = File(cacheDir, "edited-${System.currentTimeMillis()}.rpf")
            thread {
                try {
                    editor!!.saveAs(destination)
                    contentResolver.openOutputStream(uri)?.use { output -> destination.inputStream().use { it.copyTo(output, 1024 * 1024) } } ?: error("Cannot create output document")
                    runOnUiThread { status.text = "Saved new RPF • source archive untouched"; Toast.makeText(this, "New RPF saved successfully", Toast.LENGTH_LONG).show() }
                } catch (e: Exception) { runOnUiThread { status.text = "Save blocked: ${e.message}"; Toast.makeText(this, e.message ?: "Save failed", Toast.LENGTH_LONG).show() } }
            }
        } else if (requestCode == SAVE_FILE) {
            val entry = pendingExtract ?: return
            thread {
                try {
                    contentResolver.openOutputStream(uri)?.use { archive!!.extract(entry, it) } ?: error("Cannot open destination")
                    runOnUiThread { Toast.makeText(this, "Extracted ${entry.name}", Toast.LENGTH_LONG).show() }
                } catch (e: Exception) { runOnUiThread { Toast.makeText(this, "Extract failed: ${e.message}", Toast.LENGTH_LONG).show() } }
            }
        }
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
        val i = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE); type = "application/octet-stream"
            putExtra(Intent.EXTRA_TITLE, "edited-gta-v.rpf")
        }
        startActivityForResult(i, SAVE_RPF)
    }

    private fun render() {
        list.removeAllViews()
        val a = archive ?: return
        val query = search.text.toString().trim().lowercase()
        a.children(currentDir).filter { editor?.isRemoved(it.index) != true }.filter { query.isEmpty() || it.name.lowercase().contains(query) }.forEach { entry ->
            val row = TextView(this).apply {
                text = (if (entry.isDirectory) "📁  " else if (entry.isResource) "◈  " else "📄  ") + entry.name + (if (!entry.isDirectory) "   (${entry.uncompressedSize} B)" else "") + (if (editor?.isReplaced(entry.index) == true) "  • REPLACED" else "")
                textSize = 15f; setTextColor(0xFFE5EAF1.toInt()); setPadding(12,16,8,16)
                setBackgroundColor(0xFF1A2029.toInt())
                setOnClickListener {
                    if (entry.isDirectory) { currentDir = entry.index; render() }
                    else if (entry.isResource) Toast.makeText(this@MainActivity, "RSC7 resource listed; resource extraction/preview is not enabled in this first build.", Toast.LENGTH_LONG).show()
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
            }, LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = 3 })
        }
        if (list.childCount == 0) list.addView(TextView(this).apply { text="No matching entries"; setTextColor(0xFFB8C3D1.toInt()); setPadding(12,24,12,24) })
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

internal class RpfArchive private constructor(private val file: RandomAccessFile, val entries: List<RpfEntry>, val encryptionLabel: String) {
    fun children(parent: Int) = entries.filter { it.parent == parent && it.index != parent }
    fun parentOf(index: Int) = entries.firstOrNull { it.index == index }?.parent ?: 0

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

    fun extract(e: RpfEntry, output: OutputStream) {
        require(!e.isDirectory && !e.isResource) { "Resource extraction requires RSC7 decoding" }
        require(!e.encrypted) { "Encrypted file needs GTA V keys" }
        val size = if (e.storedSize == 0L) e.uncompressedSize else e.storedSize
        require(size in 0..Int.MAX_VALUE.toLong()) { "Entry is too large for this extractor" }
        synchronized(file) {
            file.seek(e.offset)
            val bytes = ByteArray(64 * 1024)
            var remaining = size
            if (e.storedSize == 0L || e.storedSize == e.uncompressedSize) {
                while (remaining > 0) { val n = file.read(bytes,0,minOf(bytes.size.toLong(),remaining).toInt()); if(n<0) error("Unexpected end of archive"); output.write(bytes,0,n); remaining-=n }
            } else {
                val compressed = ByteArray(size.toInt()); file.readFully(compressed)
                InflaterInputStream(ByteArrayInputStream(compressed), Inflater(true)).use { it.copyTo(output) }
            }
        }
    }

    companion object {
        fun open(f: File): RpfArchive {
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
                require(enc == 0L || enc == 0x4E45504FL) { "Encrypted RPF table (AES/NG) is not supported yet; only OPEN/NONE archives can be browsed" }
                val names = ByteArray(namesLen); raf.seek(namesStart); raf.readFully(names)
                val raw = ArrayList<ByteArray>(count)
                for (i in 0 until count) { val b=ByteArray(16); raf.seek(16L+i*16L); raf.readFully(b); raw.add(b) }
                fun u16(b:ByteArray,o:Int)= (b[o].toInt() and 255) or ((b[o+1].toInt() and 255) shl 8)
                fun u24(b:ByteArray,o:Int)= (b[o].toLong() and 255) or ((b[o+1].toLong() and 255) shl 8) or ((b[o+2].toLong() and 255) shl 16)
                fun u32(b:ByteArray,o:Int)= (b[o].toLong() and 255) or ((b[o+1].toLong() and 255) shl 8) or ((b[o+2].toLong() and 255) shl 16) or ((b[o+3].toLong() and 255) shl 24)
                fun name(off:Int):String { if(off !in names.indices) return "entry_$off"; var end=off; while(end<names.size && names[end].toInt()!=0) end++; return String(names,off,end-off,Charsets.UTF_8).ifBlank { "entry_$off" } }
                data class Temp(val idx:Int,val nm:String,val dir:Boolean,val res:Boolean,val off:Long,val sz:Long,val usz:Long,val crypt:Boolean,val start:Int,val cnt:Int)
                val temps=ArrayList<Temp>(count)
                for(i in 0 until count) {
                    val b=raw[i]; val second=u32(b,4)
                    if(second==0x7FFFFF00L) {
                        val childStart = u32(b,8); val childCount = u32(b,12)
                        require(childStart <= count.toLong() && childCount <= count.toLong() && childStart + childCount <= count.toLong()) { "Directory child range is invalid" }
                        temps.add(Temp(i,name(u32(b,0).toInt()),true,false,0,0,0,false,childStart.toInt(),childCount.toInt()))
                    } else if((second and 0x80000000L)==0L) {
                        val off=u24(b,5)*512L; val sz=u24(b,2); val usz=u32(b,8); val cryptType=u32(b,12)
                        require(cryptType == 0L || cryptType == 1L) { "Unknown binary encryption type" }
                        val crypt=cryptType==1L
                        temps.add(Temp(i,name(u16(b,0)),false,false,off,sz,usz,crypt,0,0))
                    } else {
                        val resourceOffset = u24(b,5) and 0x7FFFFF
                        val encodedSize = u24(b,2)
                        val absolute = resourceOffset * 512L
                        val resourceSize = if (encodedSize == 0xFFFFFFL) {
                            require(absolute + 16L <= raf.length()) { "RSC7 resource header is truncated" }
                            val h = ByteArray(16); raf.seek(absolute); raf.readFully(h)
                            (h[7].toLong() and 255L) or ((h[14].toLong() and 255L) shl 8) or
                                ((h[5].toLong() and 255L) shl 16) or ((h[2].toLong() and 255L) shl 24)
                        } else encodedSize
                        require(resourceSize >= 16L && absolute + resourceSize <= raf.length()) { "RSC7 resource data range is invalid" }
                        temps.add(Temp(i,name(u16(b,0)),false,true,absolute,resourceSize,0,false,0,0))
                    }
                }
                val parents=IntArray(count){0}
                fun assign(dir:Int, depth:Int) {
                    require(depth<256) { "Directory nesting is invalid" }
                    val t=temps[dir]; val from=t.start.coerceAtLeast(0); val to=(from+t.cnt).coerceAtMost(count)
                    for (child in from until to) { if(child==dir) continue; parents[child]=dir; if(temps[child].dir) assign(child,depth+1) }
                }
                require(temps[0].dir) { "RPF root entry must be a directory" }
                if(temps[0].dir) assign(0,0)
                val result=temps.map { t ->
                    val rawEntry = raw[t.idx]
                    val sys = if (t.res) u32(rawEntry, 8) else 0L
                    val gfx = if (t.res) u32(rawEntry, 12) else 0L
                    RpfEntry(t.idx,t.nm,parents[t.idx],t.dir,t.res,t.off,t.sz,t.usz,t.crypt,t.start,t.cnt,sys,gfx)
                }
                val dataStart=((namesStart+namesLen+511L)/512L)*512L
                result.filter { !it.isDirectory }.forEach {
                    val payloadSize = if (it.isResource) it.storedSize else if (it.storedSize == 0L) it.uncompressedSize else it.storedSize
                    require(it.offset >= 0 && it.offset + payloadSize <= raf.length()) { "Entry points outside archive" }
                }
                return RpfArchive(raf,result,if(enc==0L) "NONE" else "OPEN")
            } catch(e:Exception) { raf.close(); throw e }
        }
        private fun readU32(r:RandomAccessFile,pos:Long):Long { r.seek(pos); return (r.readUnsignedByte().toLong()) or (r.readUnsignedByte().toLong() shl 8) or (r.readUnsignedByte().toLong() shl 16) or (r.readUnsignedByte().toLong() shl 24) }
    }
}

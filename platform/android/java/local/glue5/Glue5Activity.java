package local.glue5;

import android.app.AlertDialog;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Build;
import android.os.Environment;
import android.provider.Settings;
import android.provider.OpenableColumns;
import android.os.Looper;
import android.system.ErrnoException;
import android.system.Os;
import android.widget.Toast;

import org.libsdl.app.SDLActivity;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;
import javax.xml.XMLConstants;
import javax.xml.parsers.DocumentBuilderFactory;
import org.w3c.dom.Element;
import org.w3c.dom.Node;
import org.w3c.dom.NodeList;

/** The app's activity: SDL's, with our library (libglue5.so) as the main one. */
public class Glue5Activity extends SDLActivity {
    private static final int REQUEST_TURNIP_ZIP = 47112;
    private static final int REQUEST_GRAPHICS_ARCHIVE = 47113;
    private final Handler turnipHandler = new Handler(Looper.getMainLooper());
    private boolean turnipPickerOpen = false;
    private boolean graphicsPickerOpen = false;
    private final Runnable turnipImportWatcher = new Runnable() {
        @Override public void run() { checkTurnipImportRequest(); }
    };

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        // Paths used by the optional AdrenoTools Vulkan loader.
        try {
            Os.setenv("GLUE5_NATIVE_LIB_DIR", getApplicationInfo().nativeLibraryDir, true);
            Os.setenv("GLUE5_APP_FILES_DIR", getFilesDir().getAbsolutePath(), true);
            // The native VFS reads approved loose-file replacements from this
            // separate overlay; the original GTA V data directory stays read-only.
            Os.setenv("GLUE5_GRAPHICS_MOD_DIR",
                    "/storage/emulated/0/GTA5/GraphicsMods/active", true);
            // Activate the imported custom driver before SDL starts native code.
            // An explicit GLUE5_VULKAN_DRIVER launch extra below can override this
            // (for example, "system" to recover from a bad driver).
            File turnipDir = new File(getFilesDir(), "turnip");
            File driverNameFile = new File(turnipDir, "driver_name");
            String driverName = null;
            if (driverNameFile.isFile()) {
                try (java.io.BufferedReader reader = new java.io.BufferedReader(
                        new java.io.FileReader(driverNameFile))) {
                    driverName = reader.readLine();
                } catch (Exception ignored) {}
            }
            boolean validTurnip = driverName != null && !driverName.isEmpty()
                    && driverName.equals(new File(driverName).getName())
                    && driverName.endsWith(".so")
                    && new File(turnipDir, driverName).isFile();
            Os.setenv("GLUE5_VULKAN_DRIVER", validTurnip ? "turnip" : "system", true);
            // Separate folder for user-supplied, extracted PC game data.
            // Launch-intent GLUE5_* extras below may override this default.
            Os.setenv("GLUE5_PC_DATA_DIR", "/storage/emulated/0/GTAV_PC", false);
        } catch (ErrnoException e) {
            // The system Vulkan driver remains available if path setup fails.
        }
        // The same switches as a desktop run, RUST_LOG and GLUE5_*: string
        // extras of the launch intent, set before SDL starts the native main.
        Bundle extras = getIntent().getExtras();
        if (extras != null) {
            for (String key : extras.keySet()) {
                String value = extras.getString(key);
                if (value != null && (key.equals("RUST_LOG") || key.startsWith("GLUE5_"))) {
                    try { Os.setenv(key, value, true); } catch (ErrnoException e) {}
                }
            }
        }
        super.onCreate(savedInstanceState);
        requestPublicStorageAccess();
        // The native Settings page requests the document picker through a
        // marker in this private files directory. No broad storage permission
        // is required, and the import action is available whenever needed.
        turnipHandler.postDelayed(turnipImportWatcher, 500);
    }


    /** Ask Android for access to the user-owned GTA5 folder in shared storage. */
    private void requestPublicStorageAccess() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R || Environment.isExternalStorageManager()) {
            return;
        }
        try {
            Intent intent = new Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION);
            intent.setData(Uri.parse("package:" + getPackageName()));
            startActivity(intent);
            Toast.makeText(this,
                    "Allow Glue 5: Allow access to manage all files. Then return to the game.",
                    Toast.LENGTH_LONG).show();
        } catch (Exception e) {
            Toast.makeText(this,
                    "Storage permission not opened. You can enable it in Android Settings > Apps > Glue 5 > All files access.",
                    Toast.LENGTH_LONG).show();
        }
    }

    private void checkTurnipImportRequest() {
        if (isFinishing() || turnipPickerOpen || graphicsPickerOpen) return;
        File graphicsRequest = new File(getFilesDir(), "graphics-mod-import-request");
        if (graphicsRequest.isFile() && graphicsRequest.delete()) {
            graphicsPickerOpen = true;
            openGraphicsArchivePicker();
            return;
        }
        File request = new File(getFilesDir(), "turnip-import-request");
        if (request.isFile() && request.delete()) {
            turnipPickerOpen = true;
            openTurnipPicker();
            return;
        }
        turnipHandler.postDelayed(turnipImportWatcher, 500);
    }

    private void openGraphicsArchivePicker() {
        try {
            Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            intent.addCategory(Intent.CATEGORY_OPENABLE);
            intent.setType("*/*");
            startActivityForResult(intent, REQUEST_GRAPHICS_ARCHIVE);
        } catch (Exception e) {
            graphicsPickerOpen = false;
            Toast.makeText(this, "Cannot open graphics archive picker: " + e.getMessage(), Toast.LENGTH_LONG).show();
            turnipHandler.postDelayed(turnipImportWatcher, 500);
        }
    }

    private void openTurnipPicker() {
        try {
            Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            intent.addCategory(Intent.CATEGORY_OPENABLE);
            // Some file providers label ZIPs as application/octet-stream or
            // application/x-zip-compressed. Let the user select the archive;
            // importTurnipZip validates its contents before installing it.
            intent.setType("*/*");
            startActivityForResult(intent, REQUEST_TURNIP_ZIP);
        } catch (Exception e) {
            turnipPickerOpen = false;
            Toast.makeText(this, "Cannot open file picker: " + e.getMessage(), Toast.LENGTH_LONG).show();
            turnipHandler.postDelayed(turnipImportWatcher, 500);
        }
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode == REQUEST_GRAPHICS_ARCHIVE) {
            graphicsPickerOpen = false;
            try {
                if (resultCode == RESULT_OK && data != null && data.getData() != null) {
                    String installedAt = importGraphicsArchive(data.getData());
                    Toast.makeText(this, "Graphics archive extracted to " + installedAt, Toast.LENGTH_LONG).show();
                }
            } catch (Exception e) {
                Toast.makeText(this, "Graphics archive import failed: " + e.getMessage(), Toast.LENGTH_LONG).show();
            } finally {
                turnipHandler.postDelayed(turnipImportWatcher, 500);
            }
            return;
        }
        if (requestCode != REQUEST_TURNIP_ZIP) return;
        turnipPickerOpen = false;
        try {
            if (resultCode == RESULT_OK && data != null && data.getData() != null) {
                importTurnipZip(data.getData());
                Toast.makeText(this, "Turnip driver imported. Select Turnip in Settings, then fully restart Glue 5.", Toast.LENGTH_LONG).show();
            }
        } catch (Exception e) {
            Toast.makeText(this, "Turnip import failed: " + e.getMessage(), Toast.LENGTH_LONG).show();
        } finally {
            turnipHandler.postDelayed(turnipImportWatcher, 500);
        }
    }


    /**
     * Safely unpack an OpenIV OIV (ZIP container) or graphics ZIP into a
     * separate user-owned staging directory. It never writes into game/data.
     * OpenIV install.xml instructions are retained as package content; they
     * are not executed as arbitrary file operations.
     */
    private String importGraphicsArchive(Uri uri) throws Exception {
        String displayName = "GraphicsMod";
        try (android.database.Cursor cursor = getContentResolver().query(
                uri, new String[] { OpenableColumns.DISPLAY_NAME }, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) {
                String name = cursor.getString(0);
                if (name != null && !name.trim().isEmpty()) displayName = name.trim();
            }
        }
        String lowerName = displayName.toLowerCase(java.util.Locale.ROOT);
        if (!lowerName.endsWith(".zip") && !lowerName.endsWith(".oiv")) {
            throw new Exception("Select a .zip or .oiv archive");
        }
        String folderName = displayName.replaceFirst("(?i)\\.(zip|oiv)$", "")
                .replaceAll("[^A-Za-z0-9._ -]", "_").trim();
        if (folderName.isEmpty() || folderName.equals(".") || folderName.equals("..")) folderName = "GraphicsMod";

        File root = new File(Environment.getExternalStorageDirectory(), "GTA5/GraphicsMods");
        if (!root.isDirectory() && !root.mkdirs()) {
            throw new Exception("Cannot create GTA5/GraphicsMods; enable Glue 5 all-files access");
        }
        File destination = new File(root, folderName);
        if (destination.exists()) throw new Exception("A graphics package with this name already exists; rename the ZIP and retry");
        File staging = new File(root, "." + folderName + ".installing");
        deleteTree(staging);
        if (!staging.mkdirs()) throw new Exception("Cannot create temporary graphics package folder");

        int entries = 0;
        long totalExtracted = 0;
        boolean sawContent = false;
        try (InputStream input = getContentResolver().openInputStream(uri)) {
            if (input == null) throw new Exception("Cannot read selected archive");
            try (ZipInputStream zip = new ZipInputStream(input)) {
                ZipEntry entry;
                byte[] buffer = new byte[32768];
                while ((entry = zip.getNextEntry()) != null) {
                    if (++entries > 10000) throw new Exception("Archive contains too many files");
                    String raw = entry.getName().replace('\\', '/');
                    if (raw.isEmpty() || raw.startsWith("/") || raw.matches("^[A-Za-z]:.*")) {
                        throw new Exception("Archive contains an unsafe absolute path");
                    }
                    for (String part : raw.split("/")) if (part.equals("..")) {
                        throw new Exception("Archive contains a path traversal entry");
                    }
                    File target = new File(staging, raw);
                    String canonicalRoot = staging.getCanonicalPath() + File.separator;
                    if (!target.getCanonicalPath().startsWith(canonicalRoot)) {
                        throw new Exception("Archive contains an unsafe output path");
                    }
                    if (entry.isDirectory()) {
                        if (!target.isDirectory() && !target.mkdirs()) throw new Exception("Cannot create package folder");
                        continue;
                    }
                    File parent = target.getParentFile();
                    if (parent != null && !parent.isDirectory() && !parent.mkdirs()) throw new Exception("Cannot create package subfolder");
                    long entrySize = 0;
                    try (FileOutputStream output = new FileOutputStream(target)) {
                        int count;
                        while ((count = zip.read(buffer)) != -1) {
                            entrySize += count;
                            totalExtracted += count;
                            if (entrySize > 512L * 1024L * 1024L || totalExtracted > 1536L * 1024L * 1024L) {
                                throw new Exception("Graphics archive exceeds the 1.5 GB extraction limit");
                            }
                            output.write(buffer, 0, count);
                        }
                    }
                    sawContent = true;
                }
            }
        } catch (Exception e) {
            deleteTree(staging);
            throw e;
        }
        if (!sawContent) {
            deleteTree(staging);
            throw new Exception("Archive is empty or is not a supported ZIP-based OIV package");
        }

        File overlay = new File(root, "active");
        if (!overlay.isDirectory() && !overlay.mkdirs()) throw new Exception("Cannot create graphics overlay folder");
        int installed = 0;
        int skippedArchive = 0;
        int skippedUnsupported = 0;
        File installXml = findNamedFile(staging, "install.xml");
        if (installXml != null) {
            DocumentBuilderFactory factory = DocumentBuilderFactory.newInstance();
            factory.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true);
            factory.setFeature("http://xml.org/sax/features/external-general-entities", false);
            factory.setFeature("http://xml.org/sax/features/external-parameter-entities", false);
            factory.setXIncludeAware(false);
            factory.setExpandEntityReferences(false);
            try { factory.setAttribute(XMLConstants.ACCESS_EXTERNAL_DTD, ""); } catch (Exception ignored) {}
            try { factory.setAttribute(XMLConstants.ACCESS_EXTERNAL_SCHEMA, ""); } catch (Exception ignored) {}
            org.w3c.dom.Document document;
            try (InputStream xml = new java.io.FileInputStream(installXml)) {
                document = factory.newDocumentBuilder().parse(xml);
            }
            NodeList adds = document.getElementsByTagName("add");
            for (int i = 0; i < adds.getLength(); i++) {
                Node node = adds.item(i);
                if (!(node instanceof Element)) continue;
                Element add = (Element) node;
                String source = add.getAttribute("source");
                String targetPath = add.getAttribute("destination");
                boolean insideArchive = false;
                for (Node parent = add.getParentNode(); parent != null; parent = parent.getParentNode()) {
                    if (parent instanceof Element && "archive".equalsIgnoreCase(((Element) parent).getTagName())) {
                        insideArchive = true;
                        break;
                    }
                }
                if (insideArchive) { skippedArchive++; continue; }
                if (source.isEmpty() || targetPath.isEmpty()) { skippedUnsupported++; continue; }
                File sourceFile = safePackageFile(staging, source);
                if (sourceFile == null || !sourceFile.isFile()) { skippedUnsupported++; continue; }
                String relative = normalizeGameDataPath(targetPath);
                if (relative == null || relative.toLowerCase(java.util.Locale.ROOT).endsWith(".rpf")) {
                    skippedUnsupported++;
                    continue;
                }
                File original = new File(Environment.getExternalStorageDirectory(),
                        "GTA5/game/data/" + relative);
                if (!original.isFile() || original.length() != sourceFile.length()) {
                    skippedUnsupported++;
                    continue;
                }
                File output = new File(overlay, relative);
                if (!isChildOf(overlay, output)) throw new Exception("Unsafe OIV destination path");
                File parent = output.getParentFile();
                if (parent != null && !parent.isDirectory() && !parent.mkdirs()) throw new Exception("Cannot create overlay folder");
                copyFile(sourceFile, output);
                installed++;
            }
        } else {
            // Plain ZIPs may contain loose files rooted at common/ or x64/.
            installed += importLooseZipFiles(staging, overlay);
        }

        File info = new File(staging, "GLUE5_IMPORT_INFO.txt");
        String note = "Imported archive: " + displayName + "\n"
                + "Supported loose files installed into GTA5/GraphicsMods/active: " + installed + "\n"
                + "RPF/archive instructions skipped (RPF editing is not yet supported): " + skippedArchive + "\n"
                + "Other unsupported or size-mismatched entries skipped: " + skippedUnsupported + "\n"
                + "Original GTA V game/data files were not modified. Glue 5 reads active loose-file replacements through its VFS overlay.\n";
        try (FileOutputStream output = new FileOutputStream(info)) {
            output.write(note.getBytes(java.nio.charset.StandardCharsets.UTF_8));
        }
        if (!staging.renameTo(destination)) {
            deleteTree(staging);
            throw new Exception("Could not finalize graphics package folder");
        }
        return "GTA5/GraphicsMods/" + folderName + " (installed " + installed + " loose files; " +
                skippedArchive + " RPF entries skipped)";
    }

    private static File findNamedFile(File root, String name) {
        File[] children = root.listFiles();
        if (children == null) return null;
        for (File child : children) {
            if (child.isFile() && child.getName().equalsIgnoreCase(name)) return child;
            if (child.isDirectory()) {
                File found = findNamedFile(child, name);
                if (found != null) return found;
            }
        }
        return null;
    }

    private static File safePackageFile(File root, String path) throws Exception {
        String normalized = path.replace('\\', '/');
        if (normalized.startsWith("/") || normalized.matches("^[A-Za-z]:.*")) return null;
        for (String part : normalized.split("/")) if (part.equals("..")) return null;
        File file = new File(root, normalized);
        return isChildOf(root, file) ? file : null;
    }

    private static String normalizeGameDataPath(String path) {
        String value = path.replace('\\', '/').replaceAll("^/+", "");
        if (value.matches("^[A-Za-z]:.*")) return null;
        while (value.startsWith("./")) value = value.substring(2);
        if (value.regionMatches(true, 0, "data/", 0, 5)) value = value.substring(5);
        if (value.isEmpty()) return null;
        for (String part : value.split("/")) {
            if (part.isEmpty() || part.equals(".") || part.equals("..")) return null;
        }
        String lower = value.toLowerCase(java.util.Locale.ROOT);
        if (lower.contains(".rpf/") || lower.endsWith(".rpf") || lower.startsWith("update/")) return null;
        if (!(lower.startsWith("common/") || lower.startsWith("x64/"))) return null;
        return value;
    }

    private static boolean isChildOf(File root, File candidate) throws Exception {
        String base = root.getCanonicalPath() + File.separator;
        return candidate.getCanonicalPath().startsWith(base);
    }

    private static void copyFile(File source, File destination) throws Exception {
        try (InputStream input = new java.io.FileInputStream(source);
             FileOutputStream output = new FileOutputStream(destination)) {
            byte[] buffer = new byte[32768];
            int count;
            while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
        }
    }

    private static int importLooseZipFiles(File packageRoot, File overlay) throws Exception {
        int installed = 0;
        java.util.ArrayDeque<File> pending = new java.util.ArrayDeque<>();
        pending.add(packageRoot);
        while (!pending.isEmpty()) {
            File dir = pending.removeFirst();
            File[] children = dir.listFiles();
            if (children == null) continue;
            for (File child : children) {
                if (child.isDirectory()) { pending.add(child); continue; }
                String relative = packageRoot.toURI().relativize(child.toURI()).getPath();
                String targetPath = normalizeGameDataPath(relative);
                if (targetPath == null) continue;
                File original = new File(Environment.getExternalStorageDirectory(), "GTA5/game/data/" + targetPath);
                if (!original.isFile() || original.length() != child.length()) continue;
                File output = new File(overlay, targetPath);
                if (!isChildOf(overlay, output)) continue;
                File parent = output.getParentFile();
                if (parent != null && !parent.isDirectory() && !parent.mkdirs()) continue;
                copyFile(child, output);
                installed++;
            }
        }
        return installed;
    }
    private void importTurnipZip(Uri uri) throws Exception {
        File staging = new File(getFilesDir(), "turnip-import");
        deleteTree(staging);
        if (!staging.mkdirs()) throw new Exception("Cannot create temporary driver folder");

        InputStream input = getContentResolver().openInputStream(uri);
        if (input == null) throw new Exception("Cannot read selected ZIP");
        String libraryName = null;
        boolean foundLibrary = false;
        long totalExtracted = 0;
        try (ZipInputStream zip = new ZipInputStream(input)) {
            ZipEntry entry;
            byte[] buffer = new byte[32768];
            while ((entry = zip.getNextEntry()) != null) {
                if (entry.isDirectory()) continue;
                String entryName = entry.getName().replace('\\', '/');
                // A multi-ABI archive may contain the same library name for
                // arm32/x86 and arm64. Never let a later non-arm64 entry
                // overwrite the library intended for this arm64-only APK.
                if (entryName.contains("armeabi-v7a/") || entryName.contains("x86/") ||
                        entryName.contains("x86_64/") || entryName.contains("riscv64/")) continue;
                String basename = entryName.substring(entryName.lastIndexOf('/') + 1);
                // Flatten ZIP paths to safe basenames; never use archive paths
                // as output paths.
                if (basename.isEmpty()) continue;

                if ("meta.json".equals(basename)) {
                    byte[] bytes = readZipEntry(zip, 256 * 1024);
                    try {
                        org.json.JSONObject meta = new org.json.JSONObject(new String(bytes, java.nio.charset.StandardCharsets.UTF_8));
                        String declared = meta.optString("libraryName", "");
                        if (!declared.isEmpty() && declared.equals(new File(declared).getName())
                                && declared.endsWith(".so")) {
                            libraryName = declared;
                        }
                    } catch (org.json.JSONException ignored) {
                        // A legacy ZIP without valid metadata can still use the
                        // standard libvulkan_freedreno.so filename below.
                    }
                    continue;
                }

                if (!basename.endsWith(".so")) continue;
                File target = new File(staging, basename);
                long fileSize = 0;
                try (FileOutputStream output = new FileOutputStream(target)) {
                    int count;
                    while ((count = zip.read(buffer)) != -1) {
                        fileSize += count;
                        totalExtracted += count;
                        if (fileSize > 96L * 1024L * 1024L || totalExtracted > 256L * 1024L * 1024L) {
                            throw new Exception("Driver ZIP is too large");
                        }
                        output.write(buffer, 0, count);
                    }
                }
                foundLibrary = true;
            }
        }

        if (!foundLibrary) {
            deleteTree(staging);
            throw new Exception("ZIP has no root-level Vulkan driver .so");
        }
        if (libraryName == null) {
            if (new File(staging, "libvulkan_freedreno.so").isFile()) {
                libraryName = "libvulkan_freedreno.so";
            } else {
                File[] candidates = staging.listFiles((dir, name) ->
                        name.startsWith("vulkan.ad") && name.endsWith(".so"));
                if (candidates != null && candidates.length == 1) {
                    libraryName = candidates[0].getName();
                } else {
                    File[] all = staging.listFiles((dir, name) -> name.endsWith(".so"));
                    if (all != null && all.length == 1) libraryName = all[0].getName();
                }
            }
        }
        if (libraryName == null || !new File(staging, libraryName).isFile()) {
            deleteTree(staging);
            throw new Exception("Could not identify the Vulkan driver in ZIP metadata");
        }
        File nameFile = new File(staging, "driver_name");
        try (FileOutputStream output = new FileOutputStream(nameFile)) {
            output.write(libraryName.getBytes(java.nio.charset.StandardCharsets.UTF_8));
        }

        File destination = new File(getFilesDir(), "turnip");
        deleteTree(destination);
        if (!staging.renameTo(destination)) {
            deleteTree(staging);
            throw new Exception("Cannot install driver in private storage");
        }
    }

    private static byte[] readZipEntry(ZipInputStream zip, int limit) throws Exception {
        java.io.ByteArrayOutputStream output = new java.io.ByteArrayOutputStream();
        byte[] buffer = new byte[8192];
        int total = 0;
        int count;
        while ((count = zip.read(buffer)) != -1) {
            total += count;
            if (total > limit) throw new Exception("Driver metadata is too large");
            output.write(buffer, 0, count);
        }
        return output.toByteArray();
    }

    private static void deleteTree(File file) {
        if (file.isDirectory()) {
            File[] children = file.listFiles();
            if (children != null) {
                for (File child : children) deleteTree(child);
            }
        }
        file.delete();
    }

    @Override
    protected String[] getLibraries() {
        return new String[] { "SDL3", "glue5" };
    }
}

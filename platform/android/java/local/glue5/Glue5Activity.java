package local.glue5;

import android.app.AlertDialog;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Build;
import android.os.Environment;
import android.provider.Settings;
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

/** The app's activity: SDL's, with our library (libglue5.so) as the main one. */
public class Glue5Activity extends SDLActivity {
    private static final int REQUEST_TURNIP_ZIP = 47112;
    private static final int REQUEST_MOD_ZIP = 47113;
    private final Handler turnipHandler = new Handler(Looper.getMainLooper());
    private boolean turnipPickerOpen = false;
    private String pendingModDirectory;
    private final Runnable turnipImportWatcher = new Runnable() {
        @Override public void run() { checkTurnipImportRequest(); }
    };

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        // Paths used by the optional AdrenoTools Vulkan loader.
        try {
            Os.setenv("GLUE5_NATIVE_LIB_DIR", getApplicationInfo().nativeLibraryDir, true);
            Os.setenv("GLUE5_APP_FILES_DIR", getFilesDir().getAbsolutePath(), true);
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
        if (isFinishing() || turnipPickerOpen) return;
        File request = new File(getFilesDir(), "turnip-import-request");
        if (request.isFile() && request.delete()) {
            turnipPickerOpen = true;
            openTurnipPicker();
            return;
        }
        File modRequest = new File(getFilesDir(), "mod-import-request");
        if (modRequest.isFile()) {
            try {
                pendingModDirectory = new String(java.nio.file.Files.readAllBytes(modRequest.toPath()),
                        java.nio.charset.StandardCharsets.UTF_8).trim();
            } catch (Exception e) {
                pendingModDirectory = null;
            }
            modRequest.delete();
            if (pendingModDirectory != null && !pendingModDirectory.isEmpty()) {
                turnipPickerOpen = true;
                openModPicker();
                return;
            }
        }
        turnipHandler.postDelayed(turnipImportWatcher, 500);
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

    private void openModPicker() {
        try {
            Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            intent.addCategory(Intent.CATEGORY_OPENABLE);
            intent.setType("*/*");
            startActivityForResult(intent, REQUEST_MOD_ZIP);
        } catch (Exception e) {
            turnipPickerOpen = false;
            Toast.makeText(this, "Cannot open mod ZIP picker: " + e.getMessage(), Toast.LENGTH_LONG).show();
            turnipHandler.postDelayed(turnipImportWatcher, 500);
        }
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != REQUEST_TURNIP_ZIP && requestCode != REQUEST_MOD_ZIP) return;
        turnipPickerOpen = false;
        try {
            if (resultCode == RESULT_OK && data != null && data.getData() != null) {
                if (requestCode == REQUEST_TURNIP_ZIP) {
                    importTurnipZip(data.getData());
                    Toast.makeText(this, "Turnip driver imported. Select Turnip in Settings, then fully restart Glue 5.", Toast.LENGTH_LONG).show();
                } else {
                    importModZip(data.getData(), pendingModDirectory);
                    Toast.makeText(this, "GLUE 5 mod installed. Open Mods to start it.", Toast.LENGTH_LONG).show();
                }
            }
        } catch (Exception e) {
            Toast.makeText(this, (requestCode == REQUEST_TURNIP_ZIP ? "Turnip import failed: " : "Mod import failed: ")
                    + e.getMessage(), Toast.LENGTH_LONG).show();
        } finally {
            pendingModDirectory = null;
            turnipHandler.postDelayed(turnipImportWatcher, 500);
        }
    }

    /** Installs a script mod ZIP containing exactly one main.luau entry point. */
    private void importModZip(Uri uri, String destinationPath) throws Exception {
        if (destinationPath == null || destinationPath.isEmpty()) throw new Exception("Mod folder is unavailable");
        File modsDirectory = new File(destinationPath).getCanonicalFile();
        if (!modsDirectory.isAbsolute()) throw new Exception("Invalid mod folder");
        if (!modsDirectory.exists() && !modsDirectory.mkdirs()) throw new Exception("Cannot create mods folder");

        File staging = new File(getFilesDir(), "mod-import-staging");
        deleteTree(staging);
        if (!staging.mkdirs()) throw new Exception("Cannot create temporary mod folder");

        int entries = 0;
        long totalExtracted = 0;
        try (InputStream input = getContentResolver().openInputStream(uri);
             ZipInputStream zip = new ZipInputStream(input)) {
            if (input == null) throw new Exception("Cannot read selected ZIP");
            ZipEntry entry;
            byte[] buffer = new byte[16384];
            while ((entry = zip.getNextEntry()) != null) {
                if (++entries > 512) throw new Exception("ZIP has too many files");
                String name = entry.getName().replace('\\', '/');
                if (name.startsWith("/") || name.indexOf(':') >= 0) throw new Exception("Unsafe ZIP path");
                String[] parts = name.split("/");
                for (String part : parts) {
                    if ("..".equals(part) || ".".equals(part)) throw new Exception("Unsafe ZIP path");
                }
                if (entry.isDirectory()) continue;
                String lower = name.toLowerCase(java.util.Locale.ROOT);
                if (!(lower.endsWith(".luau") || lower.endsWith(".json") || lower.endsWith(".txt"))) {
                    throw new Exception("Only .luau, .json and .txt mod files are supported");
                }
                File target = new File(staging, name).getCanonicalFile();
                String root = staging.getCanonicalPath() + File.separator;
                if (!target.getPath().startsWith(root)) throw new Exception("ZIP path escapes staging folder");
                File parent = target.getParentFile();
                if (parent != null && !parent.isDirectory() && !parent.mkdirs()) throw new Exception("Cannot create mod folder");
                long fileBytes = 0;
                try (FileOutputStream output = new FileOutputStream(target)) {
                    int count;
                    while ((count = zip.read(buffer)) != -1) {
                        fileBytes += count;
                        totalExtracted += count;
                        if (fileBytes > 8L * 1024L * 1024L || totalExtracted > 64L * 1024L * 1024L) {
                            throw new Exception("Mod ZIP exceeds the 64 MB safety limit");
                        }
                        output.write(buffer, 0, count);
                    }
                }
            }
        } catch (Exception e) {
            deleteTree(staging);
            throw e;
        }

        java.util.ArrayList<File> entryPoints = new java.util.ArrayList<>();
        findModEntryPoints(staging, entryPoints, 0);
        if (entryPoints.size() != 1) {
            deleteTree(staging);
            throw new Exception("ZIP must contain exactly one main.luau file");
        }
        File modRoot = entryPoints.get(0).getParentFile();
        String name = modRoot.equals(staging) ? modNameFromUri(uri) : safeModName(modRoot.getName());
        if (name == null || name.isEmpty()) {
            deleteTree(staging);
            throw new Exception("Cannot determine a safe mod folder name");
        }

        File manifest = new File(modRoot, "mod.json");
        if (manifest.isFile()) {
            try (InputStream in = new java.io.FileInputStream(manifest)) {
                byte[] bytes = new byte[(int) Math.min(manifest.length(), 256 * 1024)];
                int count = in.read(bytes);
                if (count < 0) throw new Exception("Empty mod.json");
                new org.json.JSONObject(new String(bytes, 0, count, java.nio.charset.StandardCharsets.UTF_8));
            } catch (Exception e) {
                deleteTree(staging);
                throw new Exception("mod.json is invalid");
            }
        }

        File destination = new File(modsDirectory, name).getCanonicalFile();
        if (!destination.getPath().startsWith(modsDirectory.getPath() + File.separator)) {
            deleteTree(staging);
            throw new Exception("Invalid mod folder name");
        }
        if (destination.exists()) {
            deleteTree(staging);
            throw new Exception("A mod named '" + name + "' is already installed");
        }
        copyModTree(modRoot, destination);
        deleteTree(staging);
        File complete = new File(getFilesDir(), "mod-import-complete");
        try (FileOutputStream output = new FileOutputStream(complete)) {
            output.write(name.getBytes(java.nio.charset.StandardCharsets.UTF_8));
        }
    }

    private static void findModEntryPoints(File directory, java.util.List<File> found, int depth) {
        if (depth > 5 || found.size() > 1) return;
        File[] children = directory.listFiles();
        if (children == null) return;
        for (File child : children) {
            if (child.isDirectory()) findModEntryPoints(child, found, depth + 1);
            else if ("main.luau".equalsIgnoreCase(child.getName())) found.add(child);
        }
    }

    private String modNameFromUri(Uri uri) throws Exception {
        String display = "imported-mod";
        try (android.database.Cursor cursor = getContentResolver().query(uri,
                new String[]{android.provider.OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) {
                int column = cursor.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME);
                if (column >= 0) display = cursor.getString(column);
            }
        }
        int dot = display.lastIndexOf('.');
        if (dot > 0) display = display.substring(0, dot);
        return safeModName(display);
    }

    private static String safeModName(String raw) {
        String name = raw.toLowerCase(java.util.Locale.ROOT).replaceAll("[^a-z0-9_-]+", "-");
        name = name.replaceAll("^-+|-+$", "");
        if (name.length() > 40) name = name.substring(0, 40);
        return name.isEmpty() ? null : name;
    }

    private static void copyModTree(File source, File destination) throws Exception {
        if (source.isDirectory()) {
            if (!destination.isDirectory() && !destination.mkdirs()) throw new Exception("Cannot create installed mod folder");
            File[] children = source.listFiles();
            if (children == null) throw new Exception("Cannot read extracted mod");
            for (File child : children) copyModTree(child, new File(destination, child.getName()));
        } else {
            File parent = destination.getParentFile();
            if (parent != null && !parent.isDirectory() && !parent.mkdirs()) throw new Exception("Cannot create installed mod folder");
            try (InputStream input = new java.io.FileInputStream(source);
                 FileOutputStream output = new FileOutputStream(destination)) {
                byte[] buffer = new byte[16384];
                int count;
                while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
            }
        }
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

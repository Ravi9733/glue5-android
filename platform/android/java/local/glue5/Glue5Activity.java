package local.glue5;

import android.app.AlertDialog;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
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
    private final Handler turnipHandler = new Handler(Looper.getMainLooper());
    private boolean turnipPickerOpen = false;
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
        // The native Settings page requests the document picker through a
        // marker in this private files directory. No broad storage permission
        // is required, and the import action is available whenever needed.
        turnipHandler.postDelayed(turnipImportWatcher, 500);
    }

    private void checkTurnipImportRequest() {
        if (isFinishing() || turnipPickerOpen) return;
        File request = new File(getFilesDir(), "turnip-import-request");
        if (request.isFile() && request.delete()) {
            turnipPickerOpen = true;
            openTurnipPicker();
            return;
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

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
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

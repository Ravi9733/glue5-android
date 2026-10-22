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
    private static final String PREFS = "glue5_driver_import";

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
        // Offer a one-time import on a clean install. The ZIP is read through
        // Android's document picker; no broad storage permission is required.
        if (!new File(getFilesDir(), "turnip/libvulkan_freedreno.so").isFile()) {
            new Handler(Looper.getMainLooper()).postDelayed(() -> showTurnipImportPrompt(), 1800);
        }
    }

    private void showTurnipImportPrompt() {
        if (isFinishing()) return;
        new AlertDialog.Builder(this)
            .setTitle("Optional Turnip Vulkan driver")
            .setMessage("Import your Turnip ZIP to enable the Turnip option in Glue 5 settings. System driver remains the default.")
            .setPositiveButton("Choose ZIP", (dialog, which) -> {
                Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                intent.setType("application/zip");
                startActivityForResult(intent, REQUEST_TURNIP_ZIP);
            })
            .setNegativeButton("Later", (dialog, which) -> { })
            .setOnCancelListener(dialog -> { })
            .show();
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != REQUEST_TURNIP_ZIP || resultCode != RESULT_OK || data == null) return;
        Uri uri = data.getData();
        if (uri == null) return;
        try {
            importTurnipZip(uri);
            Toast.makeText(this, "Turnip driver imported. Select Turnip in Settings, then restart Glue 5.", Toast.LENGTH_LONG).show();
        } catch (Exception e) {
            Toast.makeText(this, "Turnip import failed: " + e.getMessage(), Toast.LENGTH_LONG).show();
        }
    }

    private void importTurnipZip(Uri uri) throws Exception {
        File destination = new File(getFilesDir(), "turnip");
        if (!destination.exists() && !destination.mkdirs()) throw new Exception("Cannot create private driver folder");
        File temp = new File(destination, "libvulkan_freedreno.so.tmp");
        boolean found = false;
        try (InputStream input = getContentResolver().openInputStream(uri);
             ZipInputStream zip = new ZipInputStream(input)) {
            if (input == null) throw new Exception("Cannot read selected ZIP");
            ZipEntry entry;
            while ((entry = zip.getNextEntry()) != null) {
                if (!entry.isDirectory() && "libvulkan_freedreno.so".equals(new File(entry.getName()).getName())
                        && "libvulkan_freedreno.so".equals(entry.getName())) {
                    long total = 0;
                    byte[] buffer = new byte[32768];
                    try (FileOutputStream output = new FileOutputStream(temp)) {
                        int count;
                        while ((count = zip.read(buffer)) != -1) {
                            total += count;
                            if (total > 64L * 1024L * 1024L) throw new Exception("Driver file exceeds size limit");
                            output.write(buffer, 0, count);
                        }
                    }
                    if (total < 1024L * 1024L) throw new Exception("Driver file is unexpectedly small");
                    found = true;
                    break;
                }
            }
        }
        if (!found) {
            temp.delete();
            throw new Exception("ZIP does not contain libvulkan_freedreno.so at its root");
        }
        File target = new File(destination, "libvulkan_freedreno.so");
        if (target.exists() && !target.delete()) throw new Exception("Cannot replace previous driver");
        if (!temp.renameTo(target)) {
            temp.delete();
            throw new Exception("Cannot install driver in private storage");
        }
    }

    @Override
    protected String[] getLibraries() {
        return new String[] { "SDL3", "glue5" };
    }
}

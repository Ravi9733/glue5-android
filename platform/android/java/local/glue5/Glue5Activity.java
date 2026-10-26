package local.glue5;

import android.os.Bundle;
import android.os.Build;
import android.os.Environment;
import android.content.Intent;
import android.net.Uri;
import android.provider.Settings;
import android.system.ErrnoException;
import android.system.Os;

import org.libsdl.app.SDLActivity;

/** The app's activity: SDL's, with our library (libglue5.so) as the main one. */
public class Glue5Activity extends SDLActivity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        // The same switches as a desktop run, RUST_LOG and GLUE5_*: string
        // extras of the launch intent (`mise run android:run` passes them),
        // set in the environment before SDL starts the library's main.
        Bundle extras = getIntent().getExtras();
        if (extras != null) {
            for (String key : extras.keySet()) {
                String value = extras.getString(key);
                if (value != null && (key.equals("RUST_LOG") || key.startsWith("GLUE5_"))) {
                    try {
                        Os.setenv(key, value, true);
                    } catch (ErrnoException e) {
                        // Left unset.
                    }
                }
            }
        }
        super.onCreate(savedInstanceState);
        // Shared game files live in /storage/emulated/0/GTA5. Android 11+
        // requires the user to grant all-files access in system settings.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && !Environment.isExternalStorageManager()) {
            try {
                Intent intent = new Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                    Uri.parse("package:" + getPackageName()));
                startActivity(intent);
            } catch (Exception e) {
                // The user can grant access later in the app's system settings.
            }
        }
    }

    @Override
    protected String[] getLibraries() {
        return new String[] { "SDL3", "glue5" };
    }
}

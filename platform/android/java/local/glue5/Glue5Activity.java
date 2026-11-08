package local.glue5;

import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.system.ErrnoException;
import android.system.Os;

import org.libsdl.app.SDLActivity;

/**
 * Glue5 launcher. A user-selected PC game directory is retained through the
 * Storage Access Framework; native mounting consumes GLUE5_PC_GAME_TREE_URI.
 */
public class Glue5Activity extends SDLActivity {
    private static final int PICK_PC_GAME_DIRECTORY = 7314;
    private static final String PREFS = "glue5_pc_game";
    private static final String TREE_URI = "tree_uri";
    private boolean waitingForDirectory = false;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        String tree = getSharedPreferences(PREFS, MODE_PRIVATE).getString(TREE_URI, null);
        if (tree == null) {
            waitingForDirectory = true;
            Intent picker = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
            picker.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION
                    | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
            startActivityForResult(picker, PICK_PC_GAME_DIRECTORY);
            return;
        }
        setEnvironment("GLUE5_PC_GAME_TREE_URI", tree);
        setLaunchEnvironment();
        super.onCreate(savedInstanceState);
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != PICK_PC_GAME_DIRECTORY || !waitingForDirectory) return;
        waitingForDirectory = false;
        if (resultCode != RESULT_OK || data == null || data.getData() == null) {
            finish();
            return;
        }
        Uri uri = data.getData();
        int flags = data.getFlags() & (Intent.FLAG_GRANT_READ_URI_PERMISSION
                | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
        try {
            getContentResolver().takePersistableUriPermission(uri, flags);
        } catch (SecurityException ignored) {
            // The URI is still usable for this process; it may need reselection later.
        }
        getSharedPreferences(PREFS, MODE_PRIVATE).edit()
                .putString(TREE_URI, uri.toString()).apply();
        setEnvironment("GLUE5_PC_GAME_TREE_URI", uri.toString());
        setLaunchEnvironment();
        super.onCreate(null);
    }

    private void setLaunchEnvironment() {
        Bundle extras = getIntent().getExtras();
        if (extras == null) return;
        for (String key : extras.keySet()) {
            Object raw = extras.get(key);
            if (!(raw instanceof String)) continue;
            if (key.equals("RUST_LOG") || key.startsWith("GLUE5_")
                    || key.equals("VK_DRIVER_FILES") || key.equals("VK_ICD_FILENAMES")
                    || key.equals("VK_LAYER_PATH") || key.equals("VK_INSTANCE_LAYERS")) {
                setEnvironment(key, (String) raw);
            }
        }
    }

    private void setEnvironment(String key, String value) {
        try {
            Os.setenv(key, value, true);
        } catch (ErrnoException ignored) {
            // Environment variable remains unset; native startup reports this.
        }
    }

    @Override
    protected String[] getLibraries() {
        return new String[] { "SDL3", "glue5" };
    }
}

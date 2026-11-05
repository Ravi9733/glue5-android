#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

mkdir -p "$tmp/src/android/os" "$tmp/src/android/content" "$tmp/src/android/app" "$tmp/src/android/net" "$tmp/src/android/system" "$tmp/src/org/libsdl/app" "$tmp/classes"

cat >"$tmp/src/android/os/Bundle.java" <<'JAVA'
package android.os;
import java.util.Set;
public class Bundle {
    public Set<String> keySet() { return Set.of(); }
    public Object get(String key) { return null; }
}
JAVA
cat >"$tmp/src/android/content/Intent.java" <<'JAVA'
package android.content;
import android.os.Bundle;
import android.net.Uri;
public class Intent {
    public static final String ACTION_OPEN_DOCUMENT_TREE = "android.intent.action.OPEN_DOCUMENT_TREE";
    public static final int FLAG_GRANT_READ_URI_PERMISSION = 1;
    public static final int FLAG_GRANT_WRITE_URI_PERMISSION = 2;
    public static final int FLAG_GRANT_PERSISTABLE_URI_PERMISSION = 64;
    public Intent(String action) {}
    public Intent addFlags(int flags) { return this; }
    public Bundle getExtras() { return null; }
    public Uri getData() { return null; }
    public int getFlags() { return 0; }
}
JAVA
cat >"$tmp/src/android/app/Activity.java" <<'JAVA'
package android.app;
import android.content.Intent;
import android.os.Bundle;
import android.content.SharedPreferences;
import android.content.ContentResolver;
public class Activity {
    public static final int RESULT_OK = -1;
    public static final int MODE_PRIVATE = 0;
    public Intent getIntent() { return null; }
    protected void onCreate(Bundle state) {}
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {}
    public void startActivityForResult(Intent intent, int requestCode) {}
    public void finish() {}
    public SharedPreferences getSharedPreferences(String name, int mode) { return null; }
    public ContentResolver getContentResolver() { return null; }
}
JAVA
cat >"$tmp/src/android/net/Uri.java" <<'JAVA'
package android.net;
public class Uri { public String toString() { return ""; } }
JAVA
cat >"$tmp/src/android/content/SharedPreferences.java" <<'JAVA'
package android.content;
public interface SharedPreferences {
    String getString(String key, String fallback);
    Editor edit();
    interface Editor { Editor putString(String key, String value); void apply(); }
}
JAVA
cat >"$tmp/src/android/content/ContentResolver.java" <<'JAVA'
package android.content;
import android.net.Uri;
public class ContentResolver {
    public void takePersistableUriPermission(Uri uri, int flags) {}
}
JAVA
cat >"$tmp/src/android/system/ErrnoException.java" <<'JAVA'
package android.system;
public class ErrnoException extends Exception {
    private static final long serialVersionUID = 1L;
}
JAVA
cat >"$tmp/src/android/system/Os.java" <<'JAVA'
package android.system;
public final class Os {
    public static void setenv(String key, String value, boolean overwrite) throws ErrnoException {}
}
JAVA
cat >"$tmp/src/org/libsdl/app/SDLActivity.java" <<'JAVA'
package org.libsdl.app;
public class SDLActivity extends android.app.Activity {
    protected String[] getLibraries() { return new String[0]; }
}
JAVA

javac -Xlint:all -Werror -d "$tmp/classes" \
    $(find "$tmp/src" -name '*.java' -print) \
    "$root/platform/android/java/local/glue5/Glue5Activity.java"
echo "Glue5Activity.java syntax/API-shape check passed."

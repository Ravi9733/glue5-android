#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

mkdir -p "$tmp/src/android/os" "$tmp/src/android/content" "$tmp/src/android/app" "$tmp/src/android/system" "$tmp/src/org/libsdl/app" "$tmp/classes"

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
public class Intent {
    public Bundle getExtras() { return null; }
}
JAVA
cat >"$tmp/src/android/app/Activity.java" <<'JAVA'
package android.app;
import android.content.Intent;
import android.os.Bundle;
public class Activity {
    public Intent getIntent() { return null; }
    protected void onCreate(Bundle state) {}
}
JAVA
cat >"$tmp/src/android/system/ErrnoException.java" <<'JAVA'
package android.system;
public class ErrnoException extends Exception {\n    private static final long serialVersionUID = 1L;\n}
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

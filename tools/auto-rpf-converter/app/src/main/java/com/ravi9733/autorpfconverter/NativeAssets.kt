package com.ravi9733.autorpconverter
/**
 * Gen9 resource conversion through the pinned ragekit C++ library.
 * Only supported resource formats are rewritten; unrelated files are preserved.
 */
internal object NativeAssets {
    init { System.loadLibrary("gta_ragekit_jni") }
    external fun convertDirectory(rootAbsolutePath: String): String
}

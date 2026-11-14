package com.ravi9733.autorpconverter

/**
 * Thin JNI bridge. SAF documents must first be copied to app-private storage;
 * native extraction writes only to an app-private destination directory.
 */
internal object NativeRpf {
    init {
        System.loadLibrary("gta_rpf_native_core")
    }

    external fun extractArchive(sourceAbsolutePath: String, destinationAbsolutePath: String): String
    external fun buildAndroidRpf7(sourceDirectoryAbsolutePath: String, outputAbsolutePath: String): String
}

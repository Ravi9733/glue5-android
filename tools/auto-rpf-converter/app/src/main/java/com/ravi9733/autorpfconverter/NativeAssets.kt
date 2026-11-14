package com.ravi9733.autorpconverter

/**
 * Native asset and archive operations through the pinned ragekit C++ library.
 * Imported key files stay in the app-private files directory.
 */
internal object NativeAssets {
    init { System.loadLibrary("gta_ragekit_jni") }
    external fun convertDirectory(rootAbsolutePath: String): String
    external fun inspectRpfKeys(keyDirectoryAbsolutePath: String): String
    external fun extractArchiveWithKeys(
        sourceAbsolutePath: String,
        destinationAbsolutePath: String,
        keyDirectoryAbsolutePath: String
    ): String
}

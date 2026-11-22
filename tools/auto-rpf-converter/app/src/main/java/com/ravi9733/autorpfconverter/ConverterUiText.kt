package com.ravi9733.autorpconverter

/** Accurate, versioned copy shown in the Android converter UI. */
internal object ConverterUiText {
    const val VERSION = "0.3.12"
    const val SUBTITLE =
        "Android converter $VERSION • Gen9→Legacy + RPF7 + DLC/audio metadata validation"

    const val WELCOME =
        "Select a PC RPF or GTA V folder. Import matching AES/NG key files first for encrypted archives. " +
        "Gen9 YTD/YDR/YDD/YFT/YPT resources are converted to Legacy. Edition-independent YBN/YMAP/YTYP/" +
        "YCD/YND/YVR/YWR/YLD/YED/YFD/YBD/YNV resources are parser-validated and preserved byte-for-byte. " +
        "AWC/REL/DAT/GXT2/PSO/RBF/nametable containers are parser-validated and preserved; AWC codec 7 " +
        "and unsupported codecs remain opaque. These preserved formats are not counted as converted. " +
        "Unsupported files remain unchanged. Nested archives are rebuilt as OPEN RPF7. This output is " +
        "not certified as Glue5-ready game data; the compatible game.wasm and translated shaders are " +
        "separate runtime requirements."
}

package com.ravi9733.autorpconverter

/**
 * File-size and group policy for user-supplied GTA V RPF keys.
 * This validates file shape only; it never derives or guesses keys.
 */
internal object RpfKeyMaterialPolicy {
    val expectedFileSizes: Map<String, Long> = linkedMapOf(
        "gtav_aes_key.dat" to 32L,
        "gtav_ng_key.dat" to 101L * 272L,
        "gtav_ng_decrypt_tables.dat" to 17L * 16L * 256L * 4L,
        "gtav_hash_lut.dat" to 256L,
        "gtav_awc_key.dat" to 16L
    )

    private val ngFiles = setOf(
        "gtav_ng_key.dat",
        "gtav_ng_decrypt_tables.dat",
        "gtav_hash_lut.dat"
    )

    fun validate(importedSizes: Map<String, Long>) {
        require(importedSizes.isNotEmpty()) {
            "No recognized key files found. Select the folder containing gtav_aes_key.dat and/or the complete ragekit NG key set."
        }
        for ((name, size) in importedSizes) {
            val expected = expectedFileSizes[name]
                ?: throw IllegalArgumentException("Unrecognized key file: $name")
            require(size == expected) {
                "$name has an invalid size: expected $expected bytes, got $size"
            }
        }
        val ngCount = ngFiles.count { importedSizes.containsKey(it) }
        require(ngCount == 0 || ngCount == ngFiles.size) {
            "NG key import is incomplete. Select all three files: gtav_ng_key.dat, gtav_ng_decrypt_tables.dat and gtav_hash_lut.dat."
        }
    }
}

package com.ravi9733.autorpconverter

import org.junit.Assert.assertThrows
import org.junit.Test

class RpfKeyMaterialPolicyTest {
    @Test
    fun acceptsAnExactAesKeyAlone() {
        RpfKeyMaterialPolicy.validate(mapOf("gtav_aes_key.dat" to 32L))
    }

    @Test
    fun acceptsACompleteNgKeySet() {
        RpfKeyMaterialPolicy.validate(
            mapOf(
                "gtav_ng_key.dat" to 101L * 272L,
                "gtav_ng_decrypt_tables.dat" to 17L * 16L * 256L * 4L,
                "gtav_hash_lut.dat" to 256L
            )
        )
    }

    @Test
    fun acceptsAnAwcKeyAlone() {
        RpfKeyMaterialPolicy.validate(mapOf("gtav_awc_key.dat" to 16L))
    }

    @Test
    fun rejectsPartialNgKeySet() {
        assertThrows(IllegalArgumentException::class.java) {
            RpfKeyMaterialPolicy.validate(mapOf("gtav_ng_key.dat" to 101L * 272L))
        }
    }

    @Test
    fun rejectsWrongKeyLength() {
        assertThrows(IllegalArgumentException::class.java) {
            RpfKeyMaterialPolicy.validate(mapOf("gtav_aes_key.dat" to 31L))
        }
    }

    @Test
    fun rejectsUnknownKeyFile() {
        assertThrows(IllegalArgumentException::class.java) {
            RpfKeyMaterialPolicy.validate(mapOf("magic.dat" to 64L))
        }
    }

    @Test
    fun rejectsAnEmptySelection() {
        assertThrows(IllegalArgumentException::class.java) {
            RpfKeyMaterialPolicy.validate(emptyMap())
        }
    }
}

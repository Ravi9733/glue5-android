package com.ravi9733.autorpconverter

import org.junit.Assert.assertTrue
import org.junit.Test

class ConverterUiTextTest {
    @Test
    fun subtitleMatchesCurrentAppVersion() {
        assertTrue(ConverterUiText.SUBTITLE.contains(ConverterUiText.VERSION))
        assertTrue(ConverterUiText.VERSION == "0.3.16")
    }

    @Test
    fun welcomeCopyDistinguishesConversionFromValidationAndPreservation() {
        assertTrue(ConverterUiText.WELCOME.contains("Gen9 YTD/YDR/YDD/YFT/YPT resources are converted"))
        assertTrue(ConverterUiText.WELCOME.contains("parser-validated and preserved byte-for-byte"))
        assertTrue(ConverterUiText.WELCOME.contains("AWC codec 7"))
        assertTrue(ConverterUiText.WELCOME.contains("not counted as converted"))
        assertTrue(ConverterUiText.WELCOME.contains("game.wasm"))
    }

    @Test
    fun outputGuidanceRequiresASeparateStagingFolderAndMergeIntoData() {
        assertTrue(ConverterUiText.OUTPUT_GUIDANCE.contains("separate empty staging folder"))
        assertTrue(ConverterUiText.OUTPUT_GUIDANCE.contains("merge only the generated RPF files"))
        assertTrue(ConverterUiText.OUTPUT_GUIDANCE.contains("never replace the whole data/ folder"))
    }
}

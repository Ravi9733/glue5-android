package com.ravi9733.autorpconverter

import org.junit.Assert.assertTrue
import org.junit.Test

class ConverterUiTextTest {
    @Test
    fun subtitleMatchesCurrentAppVersion() {
        assertTrue(ConverterUiText.SUBTITLE.contains(ConverterUiText.VERSION))
        assertTrue(ConverterUiText.VERSION == "0.3.12")
    }

    @Test
    fun welcomeCopyDistinguishesConversionFromValidationAndPreservation() {
        assertTrue(ConverterUiText.WELCOME.contains("Gen9 YTD/YDR/YDD/YFT/YPT resources are converted"))
        assertTrue(ConverterUiText.WELCOME.contains("parser-validated and preserved byte-for-byte"))
        assertTrue(ConverterUiText.WELCOME.contains("AWC codec 7"))
        assertTrue(ConverterUiText.WELCOME.contains("not counted as converted"))
        assertTrue(ConverterUiText.WELCOME.contains("game.wasm"))
    }
}

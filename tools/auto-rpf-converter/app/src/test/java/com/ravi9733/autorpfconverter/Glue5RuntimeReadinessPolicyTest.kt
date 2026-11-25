package com.ravi9733.autorpconverter

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class Glue5RuntimeReadinessPolicyTest {
    private fun readySnapshot() = Glue5RuntimeReadinessPolicy.Snapshot(
        dataDirectoryPresent = true,
        dataStats = Glue5RuntimeReadinessPolicy.FolderStats(1200, 84, false),
        shaderDirectoryPresent = true,
        shaderStats = Glue5RuntimeReadinessPolicy.FolderStats(6615, 0, false),
        shaderIndexPresent = true,
        partialStreamMarkerPresent = false,
        gameWasmPresent = false
    )

    @Test
    fun reportsOnlyTheRequiredRuntimeLayoutAsPresent() {
        val report = Glue5RuntimeReadinessPolicy.render(readySnapshot())
        assertTrue(report.contains("RESULT: REQUIRED DIRECTORY LAYOUT PRESENT"))
        assertTrue(report.contains("data/: present"))
        assertTrue(report.contains("b/8b0b5899ed/shaders/: present"))
        assertTrue(report.contains("b/8b0b5899ed/shaders/index.json: present"))
        assertTrue(report.contains(".glue5-stream marker: absent"))
        assertTrue(report.contains("Glue5 local game-directory gate (layout::is_game_dir): MATCH"))
        assertTrue(report.contains("RPF archives found: 84"))
        assertTrue(report.contains("not a Glue5 launch or compatibility test"))
    }


    @Test
    fun missingShaderIndexFailsTheSamePredicateUsedByGlue5() {
        val report = Glue5RuntimeReadinessPolicy.render(
            readySnapshot().copy(shaderIndexPresent = false)
        )
        assertTrue(report.contains("shaders/index.json: MISSING"))
        assertTrue(report.contains("Glue5 local game-directory gate (layout::is_game_dir): DOES NOT MATCH"))
        assertTrue(report.contains("RESULT: INCOMPLETE"))
    }

    @Test
    fun partialStreamCacheMarkerIsRejectedAsALocalGameFolder() {
        val report = Glue5RuntimeReadinessPolicy.render(
            readySnapshot().copy(partialStreamMarkerPresent = true)
        )
        assertTrue(report.contains(".glue5-stream marker: PRESENT"))
        assertTrue(report.contains("partial stream cache"))
        assertTrue(report.contains("Glue5 local game-directory gate (layout::is_game_dir): DOES NOT MATCH"))
        assertTrue(report.contains("RESULT: INCOMPLETE"))
    }

    @Test
    fun missingDataDirectoryIsNotReady() {
        val snapshot = readySnapshot().copy(
            dataDirectoryPresent = false,
            dataStats = Glue5RuntimeReadinessPolicy.FolderStats(0, 0, false)
        )
        val report = Glue5RuntimeReadinessPolicy.render(snapshot)
        assertTrue(report.contains("data/: MISSING"))
        assertTrue(report.contains("RESULT: INCOMPLETE"))
    }

    @Test
    fun emptyShaderDirectoryIsNotReady() {
        val snapshot = readySnapshot().copy(
            shaderStats = Glue5RuntimeReadinessPolicy.FolderStats(0, 0, false)
        )
        val report = Glue5RuntimeReadinessPolicy.render(snapshot)
        assertTrue(report.contains("RESULT: INCOMPLETE"))
        assertTrue(report.contains("shaders/: present"))
    }

    @Test
    fun optionalGameWasmHashIsComparedWithThePinnedBuild() {
        val match = readySnapshot().copy(
            gameWasmPresent = true,
            gameWasmBytes = 61_000_000,
            gameWasmSha256 = Glue5RuntimeReadinessPolicy.EXPECTED_GAME_WASM_SHA256
        )
        assertTrue(Glue5RuntimeReadinessPolicy.render(match).contains("Build hash: MATCH"))

        val mismatch = match.copy(gameWasmSha256 = "0".repeat(64))
        val report = Glue5RuntimeReadinessPolicy.render(mismatch)
        assertTrue(report.contains("Build hash: MISMATCH"))
        assertFalse(report.contains("Build hash: MATCH"))
    }

    @Test
    fun truncatedFolderCountsAreClearlyLowerBounds() {
        val snapshot = readySnapshot().copy(
            dataStats = Glue5RuntimeReadinessPolicy.FolderStats(250_000, 12, true),
            shaderStats = Glue5RuntimeReadinessPolicy.FolderStats(6615, 0, false)
        )
        val report = Glue5RuntimeReadinessPolicy.render(snapshot)
        assertTrue(report.contains("Count is a lower bound"))
    }

    @Test
    fun missingOptionalWasmIsNotReportedAsRuntimeFolderFailure() {
        val report = Glue5RuntimeReadinessPolicy.render(readySnapshot())
        assertTrue(report.contains("game.wasm is checked by Glue5 at build time"))
        assertTrue(report.contains("RESULT: REQUIRED DIRECTORY LAYOUT PRESENT"))
    }
}

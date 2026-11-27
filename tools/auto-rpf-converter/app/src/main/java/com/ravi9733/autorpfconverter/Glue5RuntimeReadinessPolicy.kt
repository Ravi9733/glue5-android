package com.ravi9733.autorpconverter

/**
 * Checks only the documented Glue5 runtime folder layout. It deliberately does not
 * claim that RPF archives mount, assets are complete, or shaders compile on a device.
 */
internal object Glue5RuntimeReadinessPolicy {
    const val SUPPORTED_BUILD_ID = "8b0b5899ed"
    const val EXPECTED_GAME_WASM_SHA256 =
        "11ca8d2c04c5e843d18ff4aea4899d72c86973c6b031df334e67c446b2ae83e0"

    data class FolderStats(
        val fileCount: Long,
        val rpfCount: Long,
        val scanTruncated: Boolean
    )

    data class Snapshot(
        val dataDirectoryPresent: Boolean,
        val dataStats: FolderStats,
        val shaderDirectoryPresent: Boolean,
        val shaderStats: FolderStats,
        val shaderIndexPresent: Boolean,
        val partialStreamMarkerPresent: Boolean,
        val gameWasmPresent: Boolean,
        val gameWasmBytes: Long? = null,
        val gameWasmSha256: String? = null,
        val gameWasmError: String? = null
    )

    fun render(snapshot: Snapshot): String = buildString {
        val dataHasFiles = snapshot.dataDirectoryPresent && snapshot.dataStats.fileCount > 0
        val shadersHaveFiles = snapshot.shaderDirectoryPresent && snapshot.shaderStats.fileCount > 0
        // Match glue5_runtime::layout::is_game_dir exactly: data/ must be a directory,
        // the build's shader index must exist, and a partial-stream marker must be absent.
        val runtimeDirectoryGateMatches = snapshot.dataDirectoryPresent &&
            snapshot.shaderIndexPresent && !snapshot.partialStreamMarkerPresent
        // Counts are an additional completeness signal, not part of Glue5's directory predicate.
        val contentPresent = dataHasFiles && shadersHaveFiles
        val layoutPresent = runtimeDirectoryGateMatches && contentPresent

        appendLine("GLUE5 RUNTIME DATA LAYOUT CHECK")
        appendLine("Supported Glue5 build: $SUPPORTED_BUILD_ID")
        appendLine("Expected runtime paths:")
        appendLine("  data/")
        appendLine("  b/$SUPPORTED_BUILD_ID/shaders/")
        appendLine()
        appendLine("data/: " + if (snapshot.dataDirectoryPresent) "present" else "MISSING")
        appendLine("  Files found: " + snapshot.dataStats.fileCount)
        appendLine("  RPF archives found: " + snapshot.dataStats.rpfCount)
        if (snapshot.dataStats.scanTruncated) {
            appendLine("  Count is a lower bound; the $MAX_ITEMS item probe limit was reached.")
        }
        appendLine("b/$SUPPORTED_BUILD_ID/shaders/: " + if (snapshot.shaderDirectoryPresent) "present" else "MISSING")
        appendLine("  Files found: " + snapshot.shaderStats.fileCount)
        if (snapshot.shaderStats.scanTruncated) {
            appendLine("  Count is a lower bound; the $MAX_ITEMS item probe limit was reached.")
        }
        appendLine("b/$SUPPORTED_BUILD_ID/shaders/index.json: " + if (snapshot.shaderIndexPresent) "present" else "MISSING")
        appendLine(".glue5-stream marker: " + if (snapshot.partialStreamMarkerPresent) {
            "PRESENT — this is a partial stream cache, not a complete local game folder"
        } else {
            "absent"
        })
        appendLine(
            "Glue5 local game-directory gate (layout::is_game_dir): " +
                if (runtimeDirectoryGateMatches) "MATCH" else "DOES NOT MATCH"
        )
        appendLine()
        if (!snapshot.gameWasmPresent) {
            appendLine("Optional game.wasm reference: not found under b/$SUPPORTED_BUILD_ID/.")
            appendLine("game.wasm is checked by Glue5 at build time; it is not required in the runtime folder.")
        } else if (snapshot.gameWasmSha256 != null) {
            appendLine("Optional game.wasm reference: " + (snapshot.gameWasmBytes ?: "unknown") + " bytes")
            appendLine("SHA-256: " + snapshot.gameWasmSha256)
            appendLine(
                "Build hash: " +
                    if (snapshot.gameWasmSha256.equals(EXPECTED_GAME_WASM_SHA256, ignoreCase = true)) {
                        "MATCH"
                    } else {
                        "MISMATCH — this file is not the pinned supported build"
                    }
            )
        } else {
            appendLine("Optional game.wasm reference could not be verified: " + (snapshot.gameWasmError ?: "unknown read error"))
        }
        appendLine()
        appendLine(
            if (layoutPresent) {
                "RESULT: REQUIRED DIRECTORY LAYOUT AND NON-EMPTY CONTENT PRESENT"
            } else {
                "RESULT: INCOMPLETE — one or more required runtime folders are missing or empty"
            }
        )
        appendLine()
        appendLine("The directory-gate line mirrors crates/glue5-runtime/src/lib.rs layout::is_game_dir; non-empty counts are an extra inventory check.")
        appendLine("This is a read-only folder-layout check, not a Glue5 launch or compatibility test.")
        appendLine("It does not validate every game asset, prove RPF mounting, or test GPU shader compilation.")
        appendLine("Keep original game data and converter output separate; never replace a working game folder based only on this report.")
    }

    private const val MAX_ITEMS = 250_000
}

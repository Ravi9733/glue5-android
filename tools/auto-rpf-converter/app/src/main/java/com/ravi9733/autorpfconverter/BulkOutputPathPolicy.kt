package com.ravi9733.autorpconverter

import java.util.Locale

/**
 * Validates the relative RPF paths that a bulk conversion will emit into SAF.
 * Android providers and the GTA V runtime may compare names case-insensitively,
 * so two source paths that differ only by case must not target the same output.
 */
internal object BulkOutputPathPolicy {
    data class Target(val relativeDirectory: String, val fileName: String)

    fun normalizedKey(target: Target): String {
        val directory = target.relativeDirectory.replace('\\\\', '/')
        val parts = mutableListOf<String>()
        if (directory.isNotEmpty()) {
            require(!directory.startsWith('/') && !directory.endsWith('/')) {
                "Output directory must be relative and must not contain empty path components"
            }
            for (part in directory.split('/')) {
                validateSegment(part, "directory")
                parts += part
            }
        }
        validateSegment(target.fileName, "file name")
        parts += target.fileName
        return parts.joinToString("/") { it.lowercase(Locale.ROOT) }
    }

    fun caseInsensitiveCollisions(targets: Iterable<Target>): List<String> {
        val seen = HashMap<String, String>()
        val collisions = LinkedHashSet<String>()
        for (target in targets) {
            val key = normalizedKey(target)
            val display = if (target.relativeDirectory.isEmpty()) {
                target.fileName
            } else {
                target.relativeDirectory.trimEnd('/', '\\\\') + "/" + target.fileName
            }
            val previous = seen.putIfAbsent(key, display)
            if (previous != null) collisions += "$previous ↔ $display"
        }
        return collisions.toList()
    }

    private fun validateSegment(value: String, kind: String) {
        require(value.isNotBlank() && value != "." && value != "..") {
            "Unsafe output $kind rejected"
        }
        require(
            !value.contains('/') && !value.contains('\\\\') &&
                !value.contains(':') && !value.contains('\\u0000')
        ) {
            "Unsafe output $kind rejected"
        }
    }
}

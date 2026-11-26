package com.ravi9733.autorpconverter

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class BulkOutputPathPolicyTest {
    @Test
    fun sameBasenameInDifferentDlcDirectoriesIsAllowed() {
        val collisions = BulkOutputPathPolicy.caseInsensitiveCollisions(
            listOf(
                BulkOutputPathPolicy.Target("dlcpacks/pack_a", "dlc.rpf"),
                BulkOutputPathPolicy.Target("dlcpacks/pack_b", "dlc.rpf")
            )
        )
        assertTrue(collisions.isEmpty())
    }

    @Test
    fun caseOnlyDirectoryOrFilenameDifferencesAreRejected() {
        val collisions = BulkOutputPathPolicy.caseInsensitiveCollisions(
            listOf(
                BulkOutputPathPolicy.Target("DLCpacks/PackA", "DLC.rpf"),
                BulkOutputPathPolicy.Target("dlcpacks/packa", "dlc.rpf")
            )
        )
        assertEquals(1, collisions.size)
    }

    @Test
    fun exactDuplicateTargetsAreRejected() {
        val collisions = BulkOutputPathPolicy.caseInsensitiveCollisions(
            listOf(
                BulkOutputPathPolicy.Target("update/x64", "update.rpf"),
                BulkOutputPathPolicy.Target("update/x64", "update.rpf")
            )
        )
        assertEquals(1, collisions.size)
    }

    @Test
    fun windowsSeparatorsNormalizeToTheSameTarget() {
        assertEquals(
            BulkOutputPathPolicy.normalizedKey(BulkOutputPathPolicy.Target("dlcpacks\\pack", "dlc.rpf")),
            BulkOutputPathPolicy.normalizedKey(BulkOutputPathPolicy.Target("dlcpacks/pack", "dlc.rpf"))
        )
    }

    @Test
    fun traversalAndAbsoluteOrDriveQualifiedPathsAreRejected() {
        listOf("../outside", "/absolute", "C:\\outside").forEach { path ->
            val accepted = runCatching {
                BulkOutputPathPolicy.normalizedKey(BulkOutputPathPolicy.Target(path, "dlc.rpf"))
            }.isSuccess
            assertTrue("unsafe directory accepted: $path", !accepted)
        }
        listOf("../outside.rpf", "C:\\outside.rpf", "folder/name.rpf").forEach { name ->
            val accepted = runCatching {
                BulkOutputPathPolicy.normalizedKey(BulkOutputPathPolicy.Target("", name))
            }.isSuccess
            assertTrue("unsafe filename accepted: $name", !accepted)
        }
    }
}

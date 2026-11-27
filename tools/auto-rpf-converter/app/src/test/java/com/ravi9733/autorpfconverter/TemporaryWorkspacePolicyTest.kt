package com.ravi9733.autorpconverter

import java.io.File
import java.nio.file.Files
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class TemporaryWorkspacePolicyTest {
    @Test
    fun prepareReplacesStaleScratchFilesAndCleanRemovesTheWorkspace() {
        val root = Files.createTempDirectory("rpf-workspace-test").toFile()
        try {
            val workspace = File(root, "rpf-work")
            assertTrue(workspace.mkdirs())
            File(workspace, "stale-large-archive.rpf").writeText("stale")

            TemporaryWorkspacePolicy.prepare(workspace)
            assertTrue(workspace.isDirectory)
            assertTrue(workspace.listFiles().orEmpty().isEmpty())

            File(workspace, "current-output.rpf").writeText("output")
            TemporaryWorkspacePolicy.clean(workspace)
            assertFalse(workspace.exists())
        } finally {
            root.deleteRecursively()
        }
    }

    @Test
    fun prepareAndCleanRejectSymbolicLinkWorkspacesWithoutTouchingTarget() {
        val root = Files.createTempDirectory("rpf-workspace-link-test").toFile()
        try {
            val target = File(root, "real")
            assertTrue(target.mkdirs())
            val sentinel = File(target, "keep.txt")
            sentinel.writeText("keep")
            val link = File(root, "workspace-link")
            Files.createSymbolicLink(link.toPath(), target.toPath())

            assertThrows(IllegalArgumentException::class.java) {
                TemporaryWorkspacePolicy.prepare(link)
            }
            assertThrows(IllegalArgumentException::class.java) {
                TemporaryWorkspacePolicy.clean(link)
            }
            assertTrue(sentinel.exists())
            assertTrue(link.exists())
        } finally {
            root.deleteRecursively()
        }
    }
}

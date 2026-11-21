package com.ravi9733.autorpconverter

import java.io.File
import java.nio.file.Files

/**
 * Guards app-private scratch directories used for multi-gigabyte archive copies.
 * A symlink is never followed or recursively removed.
 */
internal object TemporaryWorkspacePolicy {
    fun prepare(directory: File) {
        require(!Files.isSymbolicLink(directory.toPath())) {
            "Temporary workspace path is an unexpected symbolic link: ${directory.name}"
        }
        if (directory.exists()) {
            check(directory.deleteRecursively()) {
                "Cannot clear previous temporary workspace: ${directory.name}"
            }
        }
        check(directory.mkdirs() || directory.isDirectory) {
            "Cannot create temporary workspace: ${directory.name}"
        }
        check(!Files.isSymbolicLink(directory.toPath()) && directory.isDirectory) {
            "Temporary workspace is not a safe directory: ${directory.name}"
        }
    }

    fun clean(directory: File) {
        require(!Files.isSymbolicLink(directory.toPath())) {
            "Refusing to clean a symbolic-link workspace: ${directory.name}"
        }
        if (directory.exists()) {
            check(directory.deleteRecursively()) {
                "Cannot remove temporary workspace: ${directory.name}"
            }
        }
    }
}

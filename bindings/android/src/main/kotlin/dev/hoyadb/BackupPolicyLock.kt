// API-0011 / SPEC-0036 item 6: fixed Android protected-store backup policy by location.
package dev.hoyadb

import android.content.Context
import android.system.ErrnoException
import android.system.Os
import android.system.OsConstants
import java.io.File
import java.io.FileDescriptor
import java.io.FileOutputStream
import java.nio.channels.FileLock
import java.util.concurrent.Semaphore
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Orders protected opens of hoyadb platform wrappers in one application: first a process-wide
 * semaphore, then a no-follow FileChannel lock on
 * `noBackupFilesDir/hoyadb/.hoyadb-backup-policy.lock`. The lock file is never deleted.
 * For hoyadb platform wrappers only.
 */
object BackupPolicyLock {
    private const val CHILD = "hoyadb"
    private const val NAME = ".hoyadb-backup-policy.lock"
    private val process = Semaphore(1)
    private val suffixes = listOf("", "-wal", "-shm", "-journal")

    /** Releases the FileLock, then the semaphore. Idempotent. */
    class Held internal constructor(private val stream: FileOutputStream, private val lock: FileLock) : AutoCloseable {
        private val held = AtomicBoolean(true)
        override fun close() {
            if (!held.getAndSet(false)) return
            try {
                try { lock.release() } finally { stream.close() }
            } catch (_: Exception) {
                // Closing the descriptor drops the lock; the permit is still returned.
            } finally { process.release() }
        }
        override fun toString() = "<BackupPolicyLock.Held>"
    }

    /** No acquisition timeout is specified for the semaphore or the file lock. */
    @JvmStatic fun acquire(context: Context): Held {
        process.acquireUninterruptibly()
        var descriptor: FileDescriptor? = null
        var stream: FileOutputStream? = null
        try {
            val directory = child(context.noBackupFilesDir.canonicalFile)
            descriptor = Os.open(File(directory, NAME).path, OsConstants.O_RDWR or OsConstants.O_CREAT or OsConstants.O_NOFOLLOW, 384 /* 0600 */)
            if (!OsConstants.S_ISREG(Os.fstat(descriptor).st_mode)) throw HoyaException("BackupPolicyFailure")
            stream = FileOutputStream(descriptor)
            return Held(stream, stream.channel.lock())
        } catch (error: Throwable) {
            try { if (stream != null) stream.close() else descriptor?.let { Os.close(it) } } catch (_: Exception) { }
            process.release()
            throw if (error is HoyaException) error else HoyaException("BackupPolicyFailure")
        }
    }

    /** The controlled `hoyadb` child of a canonical OS parent: created if absent, never followed. */
    internal fun child(base: File): File {
        if (!base.isDirectory) throw HoyaException("InvalidRequest")
        val directory = File(base, CHILD)
        if (!directory.exists() && !directory.mkdir()) throw HoyaException("InvalidRequest")
        if (!directory.isDirectory || directory.canonicalFile != directory.absoluteFile || directory.parentFile?.canonicalFile != base.canonicalFile) throw HoyaException("InvalidRequest")
        return directory
    }

    /**
     * Rejects when any store family file exists under the opposite policy parent's `hoyadb`
     * child. Nothing there is created, moved, deleted or opened. Call while [Held].
     */
    @JvmStatic fun requireNoOppositeStore(context: Context, name: String, backupExcluded: Boolean) {
        try {
            val opposite = File((if (backupExcluded) context.filesDir else context.noBackupFilesDir).canonicalFile, CHILD)
            for (suffix in suffixes) {
                try { Os.lstat(File(opposite, name + suffix).path) }
                catch (error: ErrnoException) {
                    if (error.errno == OsConstants.ENOENT || error.errno == OsConstants.ENOTDIR) continue
                    throw HoyaException("BackupPolicyFailure")
                }
                throw HoyaException("BackupPolicyFailure")
            }
        } catch (error: HoyaException) { throw error }
        catch (_: Exception) { throw HoyaException("BackupPolicyFailure") }
    }
}

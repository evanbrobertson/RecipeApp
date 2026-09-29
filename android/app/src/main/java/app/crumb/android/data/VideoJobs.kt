package app.crumb.android.data

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import java.util.concurrent.atomic.AtomicInteger

/** A cooking video the server is still reading: enough to ask after it again later. */
@Serializable
data class PendingJob(val id: String, val link: String, /** Epoch ms. */ val startedAt: Long) {
    fun expired(now: Long): Boolean = now - startedAt > MAX_AGE_MS

    fun encode(): String = CrumbJson.encodeToString(serializer(), this)

    companion object {
        /** A job older than this is dropped rather than resumed. */
        const val MAX_AGE_MS = 60 * 60 * 1000L

        fun decode(text: String?): PendingJob? =
            text?.let { runCatching { CrumbJson.decodeFromString(serializer(), it) }.getOrNull() }
    }
}

/** Where the pending job is kept between runs of the app. */
interface PendingJobStore {
    suspend fun load(): PendingJob?
    suspend fun save(job: PendingJob?)
}

/** How a watched video ended, for whoever wasn't waiting on it. */
sealed interface JobEnd {
    data class Saved(val result: ImportResult) : JobEnd
    data class Failed(val status: Int, val message: String) : JobEnd
}

/**
 * Cooking videos the server reads in its queue, watched for as long as the app lives, not as long
 * as the screen that asked. A job is written down when the server answers 202 and forgotten when it
 * is saved, fails, is gone (404) or is over an hour old; [resume] picks a written-down one up again
 * when the app returns to the front or the Add box opens.
 *
 * Whoever started the import waits on [track]'s answer as before. When nobody is waiting any more
 * the end goes to [addBox] if the Add box is on screen (it opens the recipe), else [background]
 * (the app shows a toast with a way to open it).
 */
class VideoJobs(
    private val api: CrumbApi,
    private val store: PendingJobStore,
    private val scope: CoroutineScope,
    private val now: () -> Long = System::currentTimeMillis,
) {
    private val pendingState = MutableStateFlow<PendingJob?>(null)

    /** The job written down, if any. */
    val pending: StateFlow<PendingJob?> = pendingState.asStateFlow()

    private val progressState = MutableStateFlow<String?>(null)

    /** The Add box's line for the job being watched ("Queued (2nd)…"); null when none is. */
    val progress: StateFlow<String?> = progressState.asStateFlow()

    private val addBoxEnds = MutableSharedFlow<JobEnd>(extraBufferCapacity = 4)
    val addBox: SharedFlow<JobEnd> = addBoxEnds.asSharedFlow()

    private val backgroundEnds = MutableSharedFlow<JobEnd>(extraBufferCapacity = 4)
    val background: SharedFlow<JobEnd> = backgroundEnds.asSharedFlow()

    private val loaded = CompletableDeferred<Unit>()
    private var watching: Pair<String, Deferred<ImportResult>>? = null
    private val waiters = AtomicInteger()

    /** Reads the written-down job (dropping an expired one). */
    suspend fun load() {
        val saved = store.load()
        if (pendingState.value == null) {
            if (saved != null && saved.expired(now())) store.save(null) else pendingState.value = saved
        }
        loaded.complete(Unit)
    }

    /**
     * A video the server has queued: write it down and wait for it. The wait outlives the
     * caller, so leaving the screen doesn't lose the video; a dropped connection for long enough
     * ends the wait with the [OfflineException] but keeps the job for [resume].
     */
    suspend fun track(link: String, job: ImportJob, progress: (String) -> Unit = {}): ImportResult {
        val pending = PendingJob(job.jobId ?: error("A job needs an id"), link, now())
        pendingState.value = pending
        store.save(pending)
        return await(watch(pending, job, progress))
    }

    /** Asks after the written-down job again, unless it is being watched already or has expired. */
    fun resume() {
        scope.launch {
            loaded.await()
            val pending = pendingState.value ?: return@launch
            if (pending.expired(now())) return@launch forget(pending.id)
            synchronized(this@VideoJobs) {
                if (watching?.let { it.first == pending.id && it.second.isActive } == true) return@launch
                watch(pending, null, {})
            }
        }
    }

    /** Signing out: no job of this account is worth asking after. */
    suspend fun clear() {
        pendingState.value = null
        progressState.value = null
        store.save(null)
    }

    private suspend fun await(job: Deferred<ImportResult>): ImportResult {
        waiters.incrementAndGet()
        try {
            return job.await()
        } finally {
            waiters.decrementAndGet()
        }
    }

    private fun watch(pending: PendingJob, first: ImportJob?, onProgress: (String) -> Unit): Deferred<ImportResult> =
        synchronized(this) {
            val job = scope.async {
                try {
                    val result = api.watchJob(pending.id, first, expired = { pending.expired(now()) }) { line ->
                        progressState.value = line
                        onProgress(line)
                    }
                    forget(pending.id)
                    end(JobEnd.Saved(result))
                    result
                } catch (e: CancellationException) {
                    throw e
                } catch (e: JobGoneException) {
                    forget(pending.id)
                    throw e
                } catch (e: ApiException) {
                    // Signed out: keep it for whoever signs in next; anything else is the job's own failure
                    if (!e.isSignedOut) {
                        forget(pending.id)
                        end(JobEnd.Failed(e.status, e.message))
                    }
                    throw e
                    // Any other failure is no connection: the job is still running on the server,
                    // so it stays written down for [resume]
                } finally {
                    progressState.value = null
                }
            }
            watching = pending.id to job
            job
        }

    /** Hands the end to whoever is left to hear it. */
    private fun end(end: JobEnd) {
        if (waiters.get() > 0) return
        (if (addBoxEnds.subscriptionCount.value > 0) addBoxEnds else backgroundEnds).tryEmit(end)
    }

    private suspend fun forget(id: String) {
        if (pendingState.value?.id != id) return
        pendingState.value = null
        store.save(null)
    }
}

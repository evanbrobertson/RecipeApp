package app.crumb.android.data

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import okhttp3.OkHttpClient
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test

private class MemoryStore(var job: PendingJob? = null) : PendingJobStore {
    override suspend fun load() = job
    override suspend fun save(job: PendingJob?) {
        this.job = job
    }
}

class PendingJobTest {
    @Test
    fun roundTripsThroughText() {
        val job = PendingJob("j1", "https://youtu.be/abc", 1_000)
        assertEquals(job, PendingJob.decode(job.encode()))
    }

    @Test
    fun garbageIsNoJob() {
        assertNull(PendingJob.decode(null))
        assertNull(PendingJob.decode("nonsense"))
        assertNull(PendingJob.decode("""{"id":1}"""))
    }

    @Test
    fun anHourOldIsExpired() {
        val job = PendingJob("j1", "https://youtu.be/abc", 1_000)
        assertFalse(job.expired(1_000 + PendingJob.MAX_AGE_MS))
        assertTrue(job.expired(1_001 + PendingJob.MAX_AGE_MS))
        assertEquals(PendingJob.MAX_AGE_MS - 500, job.remaining(1_500))
        assertEquals(1L, job.remaining(1_000 + PendingJob.MAX_AGE_MS + 5))
    }
}

@OptIn(ExperimentalCoroutinesApi::class)
class VideoJobsTest {
    private val server = MockWebServer()
    private val api = CrumbApi(OkHttpClient()) { Session(server.url("/"), "c") }
    private val store = MemoryStore()
    private var clock = 1_000_000L
    private var scope: CoroutineScope? = null

    @Before fun start() = server.start()

    @After fun stop() {
        scope?.cancel()
        server.close()
    }

    private fun json(body: String, code: Int = 200) =
        MockResponse.Builder().code(code).addHeader("Content-Type", "application/json").body(body).build()

    private fun TestScope.jobs(): VideoJobs {
        val s = CoroutineScope(SupervisorJob() + StandardTestDispatcher(testScheduler))
        scope = s
        return VideoJobs(api, store, s) { clock }
    }

    private val queued = ImportJob(jobId = "j1", status = "queued", position = 2)
    private val done = """{"id":"j1","status":"done","recipe":{"id":12,"title":"Pasta","isNew":true}}"""

    @Test
    fun aQueuedVideoIsWrittenDownThenForgottenWhenSaved() = runTest {
        val jobs = jobs()
        server.enqueue(json("""{"id":"j1","status":"running"}"""))
        server.enqueue(json(done))
        val lines = mutableListOf<String>()
        val result = jobs.track("https://youtu.be/abc", queued) { lines += it }
        assertEquals(ImportResult(12, "Pasta", true, fromVideo = true), result)
        assertEquals("Queued (2nd)…", lines.first())
        assertNull(store.job)
        assertNull(jobs.pending.value)
        assertNull(jobs.progress.value)
    }

    @Test
    fun thePendingJobIsPersistedWhileItRuns() = runTest {
        val jobs = jobs()
        server.enqueue(json(done))
        val waiting = launch { jobs.track("https://youtu.be/abc", queued) }
        testScheduler.runCurrent()
        assertEquals(PendingJob("j1", "https://youtu.be/abc", 1_000_000L), store.job)
        assertEquals("j1", jobs.pending.value?.id)
        waiting.join()
        assertNull(store.job)
    }

    @Test
    fun aFailedVideoIsForgottenAndTheWaiterHearsIt() = runTest {
        val jobs = jobs()
        server.enqueue(json("""{"id":"j1","status":"failed","statusCode":422,"message":"No recipe in that video"}"""))
        try {
            jobs.track("https://youtu.be/abc", queued)
            fail()
        } catch (e: ApiException) {
            assertEquals("No recipe in that video", e.message)
        }
        assertNull(store.job)
    }

    @Test
    fun resumeFinishesAJobFromAnEarlierRunAndTellsTheBackground() = runTest {
        store.job = PendingJob("j1", "https://youtu.be/abc", clock - 60_000)
        val jobs = jobs()
        jobs.load()
        server.enqueue(json("""{"id":"j1","status":"running"}"""))
        server.enqueue(json(done))
        val end = async(start = CoroutineStart.UNDISPATCHED) { jobs.background.first() }
        jobs.resume()
        assertEquals(JobEnd.Saved(ImportResult(12, "Pasta", true, fromVideo = true)), end.await())
        assertNull(store.job)
        assertEquals("/api/import/jobs/j1", server.takeRequest().url.encodedPath)
    }

    @Test
    fun theAddBoxHearsItWhenItIsOnScreen() = runTest {
        store.job = PendingJob("j1", "https://youtu.be/abc", clock)
        val jobs = jobs()
        jobs.load()
        server.enqueue(json(done))
        val background = mutableListOf<JobEnd>()
        scope!!.launch { jobs.background.collect { background += it } }
        val addBox = async(start = CoroutineStart.UNDISPATCHED) { jobs.addBox.first() }
        testScheduler.runCurrent()
        jobs.resume()
        assertEquals(JobEnd.Saved(ImportResult(12, "Pasta", true, fromVideo = true)), addBox.await())
        assertTrue(background.isEmpty())
    }

    @Test
    fun aCallerStillWaitingHearsItItself() = runTest {
        val jobs = jobs()
        server.enqueue(json(done))
        val background = mutableListOf<JobEnd>()
        scope!!.launch { jobs.background.collect { background += it } }
        testScheduler.runCurrent()
        jobs.track("https://youtu.be/abc", queued)
        testScheduler.runCurrent()
        assertTrue(background.isEmpty())
    }

    @Test
    fun resumeIsOnceWhileWatching() = runTest {
        store.job = PendingJob("j1", "https://youtu.be/abc", clock)
        val jobs = jobs()
        jobs.load()
        server.enqueue(json("""{"id":"j1","status":"running"}"""))
        server.enqueue(json(done))
        jobs.resume()
        testScheduler.runCurrent()
        jobs.resume()
        jobs.resume()
        jobs.pending.first { it == null }
        assertEquals(2, server.requestCount)
    }

    @Test
    fun aJobTheServerForgotIsDroppedQuietly() = runTest {
        store.job = PendingJob("gone", "https://youtu.be/abc", clock)
        val jobs = jobs()
        jobs.load()
        server.enqueue(json("""{"statusCode":404,"statusMessage":"Not Found","message":"No such job"}""", 404))
        val ends = mutableListOf<JobEnd>()
        scope!!.launch { jobs.background.collect { ends += it } }
        testScheduler.runCurrent()
        jobs.resume()
        jobs.pending.first { it == null }
        testScheduler.runCurrent()
        assertTrue(ends.isEmpty())
        assertNull(store.job)
    }

    @Test
    fun aJobOverAnHourOldIsDroppedWithoutAsking() = runTest {
        store.job = PendingJob("old", "https://youtu.be/abc", clock - PendingJob.MAX_AGE_MS - 1)
        val jobs = jobs()
        jobs.load()
        jobs.resume()
        testScheduler.runCurrent()
        assertNull(store.job)
        assertNull(jobs.pending.value)
        assertEquals(0, server.requestCount)
    }

    @Test
    fun aJobThatAgesOutWhileWatchedIsDropped() = runTest {
        store.job = PendingJob("j1", "https://youtu.be/abc", clock - PendingJob.MAX_AGE_MS + 1)
        val jobs = jobs()
        jobs.load()
        repeat(3) { server.enqueue(json("""{"id":"j1","status":"running"}""")) }
        jobs.resume()
        jobs.pending.first { it == null }
        assertNull(store.job)
    }

    @Test
    fun aResumedFailureReachesTheBackground() = runTest {
        store.job = PendingJob("j1", "https://youtu.be/abc", clock)
        val jobs = jobs()
        jobs.load()
        server.enqueue(json("""{"id":"j1","status":"failed","statusCode":422,"message":"No recipe in that video"}"""))
        val end = async(start = CoroutineStart.UNDISPATCHED) { jobs.background.first() }
        jobs.resume()
        assertEquals(JobEnd.Failed(422, "No recipe in that video"), end.await())
        assertNull(store.job)
    }

    @Test
    fun clearForgetsEverything() = runTest {
        store.job = PendingJob("j1", "https://youtu.be/abc", clock)
        val jobs = jobs()
        jobs.load()
        assertEquals("j1", jobs.pending.value?.id)
        jobs.clear()
        assertNull(store.job)
        assertNull(jobs.pending.value)
    }
}

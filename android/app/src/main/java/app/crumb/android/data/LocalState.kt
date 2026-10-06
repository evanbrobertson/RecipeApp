package app.crumb.android.data

import android.content.Context
import androidx.compose.runtime.mutableStateMapOf
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer

// What the web keeps in browser storage (web/src/lib/storage.ts, random.ts), on the phone.
// The web's sessionStorage (per tab) becomes per process here: scale, cook step, cook timers and
// ticked ingredients, prep progress and Surprise me's recent picks last until the app is closed. Its localStorage (recently
// viewed, Home's Try next toggle) is saved in DataStore and survives restarts.

/**
 * Per-recipe state shared by the recipe, cook and prep screens: the scale (½×, 1×, 2×, 3×),
 * the cook-mode step, its timer lengths and ticked ingredients, and which prep items are ready. Compose state, so screens recompose.
 */
object RecipeSession {
    private val scales = mutableStateMapOf<Long, Double>()
    private val steps = mutableStateMapOf<Long, Int>()
    private val timers = mutableStateMapOf<Long, Map<Int, Int>>()
    private val checked = mutableStateMapOf<Long, Set<Int>>()
    private val prep = mutableStateMapOf<Long, Set<String>>()

    fun scale(id: Long): Double = scales[id] ?: 1.0
    fun setScale(id: Long, scale: Double) {
        scales[id] = scale
    }

    /** The step cook mode is on for this recipe (0-based), if it has been opened. */
    fun cookStep(id: Long): Int? = steps[id]
    fun setCookStep(id: Long, step: Int) {
        steps[id] = step
    }

    /** The timer lengths the cook changed in cook mode, in seconds by step (0-based). */
    fun cookTimers(id: Long): Map<Int, Int> = timers[id].orEmpty()
    fun setCookTimer(id: Long, step: Int, seconds: Int) {
        timers[id] = cookTimers(id) + (step to seconds)
    }

    /** The ingredients ticked in cook mode, by index (0-based). */
    fun cookChecked(id: Long): Set<Int> = checked[id].orEmpty()
    fun setCookChecked(id: Long, ticked: Set<Int>) {
        checked[id] = ticked
    }

    fun prepReady(id: Long): Set<String> = prep[id].orEmpty()
    fun setPrepReady(id: Long, ready: Set<String>) {
        prep[id] = ready
    }

    /** The scale presets, labelled "½×", "1×", "2×", "3×" (components/ScaleControl.svelte). */
    val Presets = listOf(0.5 to "½×", 1.0 to "1×", 2.0 to "2×", 3.0 to "3×")
}

/** A recipe opened recently, for Home's "Pick up where you left off". */
@Serializable
data class Viewed(
    val id: Long,
    val title: String,
    val image: String? = null,
    /** When it was last opened (epoch ms). */
    val at: Long? = null,
    /** Number of steps, for "Step 4 of 9" while cooking. */
    val steps: Int? = null,
)

/**
 * Where the floating video player was left, in dp on screen (the web's `crumb:video-window`):
 * its left and top edge and its width; the height follows from the picture's shape.
 */
@Serializable
data class VideoWin(val x: Float, val y: Float, val w: Float)

private val Context.localData: DataStore<Preferences> by preferencesDataStore(name = "local")

/** Recently viewed recipes and small UI preferences, kept on this phone only. */
class LocalStore(private val context: Context, private val scope: CoroutineScope) {
    private val recent = MutableStateFlow<List<Viewed>>(emptyList())
    val recentlyViewed: StateFlow<List<Viewed>> = recent.asStateFlow()

    private val nextOpen = MutableStateFlow(false)
    /** Home's "What should I cook next?" is expanded (crumb:home:next-open). */
    val tryNextOpen: StateFlow<Boolean> = nextOpen.asStateFlow()

    private val pinned = MutableStateFlow(true)
    /** Cook mode keeps the ingredients beside the steps on wide screens (crumb:cook-pinned). */
    val cookPinned: StateFlow<Boolean> = pinned.asStateFlow()

    /** Surprise me's recent picks this session (crumb:random:seen), newest first, up to 40. */
    private val randomSeen = ArrayDeque<Long>()

    /** The floating player's place and size, one for wide videos and one for tall ones. */
    private val windows = mutableMapOf<Boolean, VideoWin>()

    fun videoWindow(tall: Boolean): VideoWin? = windows[tall]

    fun setVideoWindow(tall: Boolean, win: VideoWin) {
        windows[tall] = win
        val json = CrumbJson.encodeToString(VideoWin.serializer(), win)
        scope.launch { context.localData.edit { it[windowKey(tall)] = json } }
    }

    suspend fun load() {
        val prefs = context.localData.data.first()
        recent.value = prefs[RECENT]?.let {
            runCatching { CrumbJson.decodeFromString(ListSerializer(Viewed.serializer()), it) }.getOrNull()
        }.orEmpty()
        nextOpen.value = prefs[NEXT_OPEN] ?: false
        pinned.value = prefs[COOK_PINNED] ?: true
        for (tall in listOf(false, true)) {
            prefs[windowKey(tall)]?.let {
                runCatching { CrumbJson.decodeFromString(VideoWin.serializer(), it) }.getOrNull()
            }?.let { windows[tall] = it }
        }
    }

    /** Most recent first, six at most, like rememberViewed() in storage.ts. */
    fun rememberViewed(recipe: Recipe) {
        val steps = recipe.instructions.sumOf { it.items.size }
        recent.value = (listOf(Viewed(recipe.id, recipe.title, recipe.image, System.currentTimeMillis(), steps)) +
            recent.value.filter { it.id != recipe.id }).take(6)
        saveRecent()
    }

    fun forgetViewed(ids: Collection<Long>) {
        recent.value = recent.value.filter { it.id !in ids }
        saveRecent()
    }

    fun setTryNextOpen(open: Boolean) {
        nextOpen.value = open
        scope.launch { context.localData.edit { it[NEXT_OPEN] = open } }
    }

    fun setCookPinned(on: Boolean) {
        pinned.value = on
        scope.launch { context.localData.edit { it[COOK_PINNED] = on } }
    }

    fun rememberRandom(id: Long) {
        randomSeen.remove(id)
        randomSeen.addFirst(id)
        while (randomSeen.size > 40) randomSeen.removeLast()
    }

    /** What Surprise me should skip: this session's picks and anything opened recently. */
    fun randomExclusions(): List<Long> = (randomSeen + recent.value.map { it.id }).distinct()

    private fun saveRecent() {
        val json = CrumbJson.encodeToString(ListSerializer(Viewed.serializer()), recent.value)
        scope.launch { context.localData.edit { it[RECENT] = json } }
    }

    private fun windowKey(tall: Boolean) = stringPreferencesKey(if (tall) "video_window_tall" else "video_window")

    private companion object {
        val RECENT = stringPreferencesKey("recent")
        val NEXT_OPEN = booleanPreferencesKey("home_next_open")
        val COOK_PINNED = booleanPreferencesKey("cook_pinned")
    }
}

/** The cooking video the server is still reading, kept in DataStore so a closed app can ask after it. */
class LocalPendingJobs(private val context: Context) : PendingJobStore {
    override suspend fun load(): PendingJob? = PendingJob.decode(context.localData.data.first()[PENDING_JOB])

    override suspend fun save(job: PendingJob?) {
        context.localData.edit { if (job == null) it.remove(PENDING_JOB) else it[PENDING_JOB] = job.encode() }
    }

    private companion object {
        val PENDING_JOB = stringPreferencesKey("pending_video_job")
    }
}

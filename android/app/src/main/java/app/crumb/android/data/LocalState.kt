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
// The web's sessionStorage (per tab) becomes per process here: scale, cook step, prep progress
// and Surprise me's recent picks last until the app is closed. Its localStorage (recently
// viewed, Home's Try next toggle) is saved in DataStore and survives restarts.

/**
 * Per-recipe state shared by the recipe, cook and prep screens: the scale (½×, 1×, 2×, 3×),
 * the cook-mode step, and which prep items are ready. Compose state, so screens recompose.
 */
object RecipeSession {
    private val scales = mutableStateMapOf<Long, Double>()
    private val steps = mutableStateMapOf<Long, Int>()
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

private val Context.localData: DataStore<Preferences> by preferencesDataStore(name = "local")

/** Recently viewed recipes and small UI preferences, kept on this phone only. */
class LocalStore(private val context: Context, private val scope: CoroutineScope) {
    private val recent = MutableStateFlow<List<Viewed>>(emptyList())
    val recentlyViewed: StateFlow<List<Viewed>> = recent.asStateFlow()

    private val nextOpen = MutableStateFlow(false)
    /** Home's "What should I cook next?" is expanded (crumb:home:next-open). */
    val tryNextOpen: StateFlow<Boolean> = nextOpen.asStateFlow()

    /** Surprise me's recent picks this session (crumb:random:seen), newest first, up to 40. */
    private val randomSeen = ArrayDeque<Long>()

    suspend fun load() {
        val prefs = context.localData.data.first()
        recent.value = prefs[RECENT]?.let {
            runCatching { CrumbJson.decodeFromString(ListSerializer(Viewed.serializer()), it) }.getOrNull()
        }.orEmpty()
        nextOpen.value = prefs[NEXT_OPEN] ?: false
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

    private companion object {
        val RECENT = stringPreferencesKey("recent")
        val NEXT_OPEN = booleanPreferencesKey("home_next_open")
    }
}

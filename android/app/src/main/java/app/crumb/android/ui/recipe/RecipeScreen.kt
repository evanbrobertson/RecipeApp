package app.crumb.android.ui.recipe

import android.content.Context
import android.content.Intent
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.net.toUri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import app.crumb.android.AppContainer
import app.crumb.android.data.ApiException
import app.crumb.android.data.CookbookListItem
import app.crumb.android.data.Durations
import app.crumb.android.data.Recipe
import app.crumb.android.data.RecipeChecks
import app.crumb.android.data.RecipeSession
import app.crumb.android.data.Share
import app.crumb.android.data.ShareKind
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnSize
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.Card
import app.crumb.android.ui.components.CrumbMenu
import app.crumb.android.ui.components.CrumbModal
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.Loading
import app.crumb.android.ui.components.MenuItem
import app.crumb.android.ui.components.Message
import app.crumb.android.ui.components.OfflineNote
import app.crumb.android.ui.components.RecipePhoto
import app.crumb.android.ui.components.Toast
import app.crumb.android.ui.components.ToastAction
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.books.bookLook
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.DmSerif
import app.crumb.android.ui.theme.NunitoSans
import app.crumb.core.hostOf
import app.crumb.core.kicker
import app.crumb.core.scaleIngredient
import app.crumb.core.webLink
import com.composables.icons.lucide.Check
import com.composables.icons.lucide.ChefHat
import com.composables.icons.lucide.ClipboardCheck
import com.composables.icons.lucide.Clock
import com.composables.icons.lucide.Dices
import com.composables.icons.lucide.ExternalLink
import com.composables.icons.lucide.Flame
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Pencil
import com.composables.icons.lucide.Plus
import com.composables.icons.lucide.Share
import com.composables.icons.lucide.Snowflake
import com.composables.icons.lucide.Soup
import com.composables.icons.lucide.StickyNote
import com.composables.icons.lucide.Timer
import com.composables.icons.lucide.Trash2
import com.composables.icons.lucide.Users
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

private const val AskWaitMs = 5 * 60_000L

/** Everything the recipe page shows: the recipe plus the things loaded alongside it. */
data class RecipePageState(
    val recipe: Recipe? = null,
    val loading: Boolean = true,
    val offline: Boolean = false,
    val error: String? = null,
    val signedOut: Boolean = false,
    val cookbooks: List<CookbookListItem> = emptyList(),
    val inCookbooks: Set<Long> = emptySet(),
    val checks: RecipeChecks? = null,
    val weeChefChecks: Boolean = false,
    val share: Share? = null,
    val cookStats: CookStats? = null,
)

/**
 * The recipe page's state (web/src/islands/RecipePage.svelte). The recipe comes through
 * container.recipes.recipe(id), so a cached copy opens offline; the cookbooks, membership,
 * Wee Chef check and connector flag load beside it and fail quietly. Background polling of a
 * pending check mirrors the web's `$effect` (1.5s first, then 3s × 10, then 10s).
 */
class RecipePageViewModel(
    private val container: AppContainer,
    private val id: Long,
) : ViewModel() {
    private val _state = MutableStateFlow(RecipePageState())
    val state: StateFlow<RecipePageState> = _state.asStateFlow()

    private var loadJob: Job? = null
    private var pollJob: Job? = null

    private var polls = 0
    private var asked = false
    private var askedAt = 0L
    private var fixedBefore: Set<Long> = emptySet()

    init {
        load()
    }

    private var shown = false

    /** The page is showing again; reload unless this is the first time (init already did). */
    fun resumed() {
        if (shown) load() else shown = true
    }

    fun load() {
        loadJob?.cancel()
        _state.update { it.copy(loading = it.recipe == null, error = null) }
        loadJob = viewModelScope.launch {
            try {
                val loaded = container.recipes.recipe(id)
                _state.update {
                    it.copy(recipe = loaded.value, offline = loaded.offline, loading = false, error = null, signedOut = false)
                }
                loadAux()
                if (_state.value.checks?.status == "pending") startPolling()
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _state.update {
                    it.copy(loading = false, error = e.friendlyMessage(), signedOut = (e as? ApiException)?.isSignedOut == true)
                }
            }
        }
    }

    private suspend fun loadAux() {
        val books = runCatching { container.recipes.cookbooks() }.getOrNull()
        val inBooks = runCatching { container.api.recipeCookbooks(id) }.getOrNull()
        val checks = runCatching { container.api.recipeChecks(id) }.getOrNull()
        val wee = runCatching { container.api.connector().weeChefChecks }.getOrNull()
        // Servers older than the Android app have no GET here; the line just stays hidden
        val cooked = runCatching { container.api.cookStats(id) }.getOrNull()
        val share = runCatching { container.api.existingShare(ShareKind.Recipe, id) }.getOrNull()
        _state.update {
            it.copy(
                cookbooks = books?.value ?: it.cookbooks,
                offline = it.offline || books?.offline == true,
                inCookbooks = inBooks?.toSet() ?: it.inCookbooks,
                checks = checks ?: it.checks,
                weeChefChecks = wee ?: it.weeChefChecks,
                cookStats = cooked?.let { c -> CookStats(c.count, c.lastCookedAt) } ?: it.cookStats,
                share = share ?: it.share,
            )
        }
    }

    fun setShare(share: Share?) = _state.update { it.copy(share = share) }

    fun undoChecks() {
        viewModelScope.launch {
            try {
                val result = container.api.undoChecks(id)
                _state.update { it.copy(recipe = result.recipe, checks = result.checks ?: it.checks) }
                Toaster.show("Put back as it was imported")
            } catch (e: Exception) {
                Toaster.show("Couldn't undo", e.friendlyMessage(), ToastTone.Error)
            }
        }
    }

    fun markCooked() {
        viewModelScope.launch {
            try {
                val cooked = container.api.markCooked(id)
                _state.update { it.copy(cookStats = CookStats(cooked.count, cooked.lastCookedAt)) }
                val eventId = cooked.eventId
                if (eventId == null) {
                    Toaster.show("Already marked as cooked", "Logged in the last few hours.")
                } else {
                    Toaster.show(
                        Toast(
                            title = "Marked as cooked",
                            description = "It'll sit out of Try next for a couple of weeks.",
                            tone = ToastTone.Success,
                            action = ToastAction("Undo") { undoCooked(eventId) },
                        ),
                    )
                }
            } catch (e: Exception) {
                Toaster.show("Couldn't mark as cooked", e.friendlyMessage(), ToastTone.Error)
            }
        }
    }

    private fun undoCooked(eventId: Long) {
        viewModelScope.launch {
            try {
                val stats = container.api.undoCooked(id, eventId)
                _state.update { it.copy(cookStats = CookStats(stats.count, stats.lastCookedAt)) }
            } catch (e: Exception) {
                Toaster.show("Couldn't undo", e.friendlyMessage(), ToastTone.Error)
            }
        }
    }

    fun toggleCookbook(bookId: Long) {
        val inBook = bookId in _state.value.inCookbooks
        viewModelScope.launch {
            try {
                if (inBook) container.api.removeFromCookbook(bookId, id) else container.api.addToCookbook(bookId, listOf(id))
                _state.update { it.copy(inCookbooks = if (inBook) it.inCookbooks - bookId else it.inCookbooks + bookId) }
            } catch (e: Exception) {
                Toaster.show("Couldn't update cookbook", e.friendlyMessage(), ToastTone.Error)
            }
        }
    }

    /** "Check with Wee Chef": ask now, then watch for the result (web `checkNow`). */
    fun checkNow() {
        viewModelScope.launch {
            val before = _state.value.checks?.flags.orEmpty()
            try {
                val checks = container.api.checkRecipe(id) ?: return@launch
                fixedBefore = before.filter { it.state == "fixed" }.map { it.id }.toSet()
                polls = 0
                asked = true
                askedAt = System.currentTimeMillis()
                _state.update { it.copy(checks = checks) }
                Toaster.show("Wee Chef is checking…")
                startPolling()
            } catch (e: Exception) {
                Toaster.show("Couldn't start the check", e.friendlyMessage(), ToastTone.Error)
            }
        }
    }

    suspend fun delete(): Boolean = try {
        container.api.deleteRecipe(id)
        true
    } catch (e: Exception) {
        Toaster.show("Couldn't delete", e.friendlyMessage(), ToastTone.Error)
        false
    }

    private fun startPolling() {
        if (_state.value.checks?.status != "pending") return
        if (pollJob?.isActive == true) return
        pollJob = viewModelScope.launch {
            while (isActive) {
                delay(if (!asked) 1500L else if (polls < 10) 3000L else 10_000L)
                if (asked) {
                    if (System.currentTimeMillis() - askedAt > AskWaitMs) {
                        asked = false
                        return@launch
                    }
                } else if (++polls > 20) {
                    return@launch
                }
                if (asked) polls++
                // A failed poll stops the loop, as on the web
                val checks = runCatching { container.api.recipeChecks(id) }.getOrNull() ?: return@launch
                if (checks.status != "pending" && checks.flags.any { it.state == "fixed" }) {
                    runCatching { container.recipes.recipe(id) }.getOrNull()?.let { loaded ->
                        _state.update { it.copy(recipe = loaded.value) }
                    }
                }
                _state.update { it.copy(checks = checks) }
                if (checks.status == "pending") continue
                if (asked) {
                    asked = false
                    val review = checks.flags.count { it.state == "review" }
                    val fixed = newlyFixedCount(checks.flags, fixedBefore)
                    when {
                        checks.status == "failed" -> Toaster.show("Wee Chef couldn't check this recipe", tone = ToastTone.Error)
                        fixed > 0 && review > 0 -> Toaster.show(tidiedTitle(fixed), mightNeedALook(review))
                        fixed > 0 -> Toaster.show(tidiedTitle(fixed))
                        review > 0 -> Toaster.show(mightNeedALook(review))
                        else -> Toaster.show("Wee Chef found nothing to change")
                    }
                }
                return@launch
            }
        }
    }
}

/**
 * One recipe (web/src/islands/RecipePage.svelte): hero, kicker/title with share and ⋯, the
 * byline, Wee Chef's note, times, the two mode buttons, the ingredients checklist and the
 * method, then notes, nutrition and the cookbooks it sits in.
 */
@Composable
fun RecipeScreen(id: Long, fromRandom: Boolean = false, onSignedOut: () -> Unit) {
    val vm = crumbViewModel(key = "recipe-page-$id") { RecipePageViewModel(it, id) }
    val state by vm.state.collectAsStateWithLifecycle()
    val container = AppContainerProvider
    val nav = LocalNav.current
    val recipe = state.recipe

    // Coming back from the editor (or anywhere) shows the saved recipe, as the web's page load does
    LifecycleResumeEffect(Unit) {
        vm.resumed()
        onPauseOrDispose {}
    }

    LaunchedEffect(state.signedOut) {
        if (state.signedOut) onSignedOut()
    }

    // A refreshed copy (Wee Chef's fix, an undo) isn't a new view
    LaunchedEffect(recipe?.id) {
        val r = recipe ?: return@LaunchedEffect
        container.local.rememberViewed(r)
        runCatching { container.api.logViewed(r.id) }
    }

    Box(Modifier.fillMaxSize()) {
        when {
            recipe == null && state.loading -> Loading()
            recipe == null -> Message(
                "Couldn't open this recipe",
                state.error,
                action = { Btn("Try again", { vm.load() }, style = BtnStyle.Outline) },
            )
            else -> RecipeContent(recipe, state, fromRandom, vm)
        }
        FilledTonalIconButton(
            onClick = { nav.back() },
            colors = IconButtonDefaults.filledTonalIconButtonColors(containerColor = Crumb.colors.paper.copy(alpha = 0.92f)),
            modifier = Modifier.statusBarsPadding().padding(12.dp).testTag("back"),
        ) {
            Icon(Icons.AutoMirrored.Outlined.ArrowBack, contentDescription = "Back")
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun RecipeContent(recipe: Recipe, state: RecipePageState, fromRandom: Boolean, vm: RecipePageViewModel) {
    val c = Crumb.colors
    val nav = LocalNav.current
    val container = AppContainerProvider
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val widthPx = LocalWindowInfo.current.containerSize.width
    val photo = container.recipes.photoUrl(recipe.id, recipe.image, widthPx)
    val scale = RecipeSession.scale(recipe.id)

    var showShare by rememberSaveable { mutableStateOf(false) }
    var showDelete by rememberSaveable { mutableStateOf(false) }
    var ticked by rememberSaveable(recipe.id) { mutableStateOf(setOf<String>()) }

    val hasHero = !recipe.image.isNullOrBlank()
    val sub = kicker(recipe.recipeCategory, recipe.recipeCuisine)
    val sourceHost = hostOf(recipe.originalUrl ?: recipe.url)
    val cooked = cookedLine(state.cookStats)
    val scaledYield = recipe.recipeYield?.takeIf { it.isNotBlank() }
        ?.let { if (scale == 1.0) it else scaleIngredient(it, scale) }
    val times = listOfNotNull(
        Durations.display(recipe.prepTime)?.let { TimesEntry("Prep", Lucide.Timer, it) },
        Durations.display(recipe.cookTime)?.let { TimesEntry("Cook", Lucide.Flame, it) },
        Durations.display(recipe.freezeTime)?.let { TimesEntry("Extra", Lucide.Snowflake, it) },
        Durations.display(recipe.totalTime)?.let { TimesEntry("Total", Lucide.Clock, it) },
    ).toMutableList()
    scaledYield?.let { times += TimesEntry("Serves", Lucide.Users, it) }
    val hasIngredients = recipe.ingredients.any { it.items.isNotEmpty() }

    LazyColumn(Modifier.fillMaxSize().testTag("recipe"), horizontalAlignment = Alignment.CenterHorizontally) {
        item {
            if (hasHero) {
                Box(Modifier.fillMaxWidth().statusBarsPadding().padding(horizontal = 16.dp, vertical = 8.dp)) {
                    RecipePhoto(photo, recipe.image, Modifier.fillMaxWidth().aspectRatio(4f / 3f), contentDescription = recipe.title)
                }
            } else {
                Box(Modifier.statusBarsPadding().padding(top = 56.dp))
            }
        }
        item {
            Column(Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 16.dp)) {
                if (state.offline) OfflineNote(Modifier.padding(bottom = 12.dp))

                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
                    Column(Modifier.weight(1f)) {
                        if (sub.isNotBlank()) {
                            Text(sub.uppercase(), style = CrumbText.kicker, color = c.primary, modifier = Modifier.padding(bottom = 8.dp))
                        }
                        Text(recipe.title, style = CrumbText.pageTitle, color = c.ink)
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(start = 12.dp)) {
                        Btn(null, { showShare = true }, style = BtnStyle.Outline, icon = Lucide.Share, contentDescription = "Share")
                        CrumbMenu(
                            groups = recipeMenu(
                                state = state,
                                vm = vm,
                                onEdit = { nav.edit(recipe.id) },
                                onSurprise = { nav.surprise(recipe.id) },
                                onDelete = { showDelete = true },
                            ),
                        )
                    }
                }

                Byline(recipe, cooked, sourceHost) { url -> openUrl(context, url) }


                recipe.description?.takeIf { it.isNotBlank() }?.let { description ->
                    Text(
                        description,
                        style = CrumbText.body.copy(fontSize = 17.sp, lineHeight = 26.sp),
                        color = c.inkMuted,
                        modifier = Modifier.padding(top = 16.dp),
                    )
                }

                state.checks?.let { checks ->
                    WeeChefCard(
                        checks = checks,
                        onUndo = { vm.undoChecks() },
                        onEdit = { nav.edit(recipe.id) },
                        modifier = Modifier.padding(top = 16.dp),
                    )
                }
                if (times.isNotEmpty()) {
                    TimesCard(times, Modifier.padding(top = 16.dp))
                }

                Row(Modifier.fillMaxWidth().padding(top = 20.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Btn("Cooking mode", { nav.cook(recipe.id) }, style = BtnStyle.Primary, size = BtnSize.Xl, icon = Lucide.Flame, padding = 12.dp, modifier = Modifier.weight(1f))
                    Btn("Mise en place", { nav.prep(recipe.id) }, style = BtnStyle.Tile, size = BtnSize.Xl, icon = Lucide.Soup, padding = 12.dp, modifier = Modifier.weight(1f))
                }
                if (fromRandom) {
                    Btn(
                        "Shuffle again",
                        { nav.surprise(recipe.id) },
                        style = BtnStyle.Soft,
                        icon = Lucide.Dices,
                        modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
                    )
                }
            }
        }

        item {
            Row(
                Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 28.dp, bottom = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text("Ingredients", style = CrumbText.sectionTitle, color = c.ink, modifier = Modifier.weight(1f))
                if (hasIngredients) ScaleControl(scale, onChange = { RecipeSession.setScale(recipe.id, it) })
            }
        }
        if (!hasIngredients) {
            item {
                Text("No ingredients listed.", style = CrumbText.bodySmall, color = c.inkMuted, modifier = Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(horizontal = 20.dp))
            }
        }
        recipe.ingredients.forEachIndexed { sectionIndex, section ->
            if (section.items.isEmpty()) return@forEachIndexed
            item(key = "ingredients-$sectionIndex") {
                Column(Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(horizontal = 20.dp)) {
                    section.name?.takeIf { it.isNotBlank() }?.let { name ->
                        Text(name.uppercase(), style = CrumbText.kicker, color = c.primary, modifier = Modifier.padding(bottom = 10.dp))
                    }
                    Card(Modifier.fillMaxWidth()) {
                        section.items.forEachIndexed { itemIndex, line ->
                            if (itemIndex > 0) HorizontalDivider(Modifier.padding(horizontal = 16.dp), color = c.line)
                            val key = "$sectionIndex:$itemIndex"
                            val done = key in ticked
                            IngredientRow(line, done, scale, onToggle = { ticked = if (done) ticked - key else ticked + key })
                        }
                    }
                }
            }
        }

        item {
            Text(
                "Method",
                style = CrumbText.sectionTitle,
                color = c.ink,
                modifier = Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 28.dp, bottom = 8.dp),
            )
        }
        if (recipe.instructions.none { it.items.isNotEmpty() }) {
            item {
                Text("No steps listed.", style = CrumbText.bodySmall, color = c.inkMuted, modifier = Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(horizontal = 20.dp))
            }
        }
        recipe.instructions.forEachIndexed { sectionIndex, section ->
            val base = recipe.instructions.take(sectionIndex).sumOf { it.items.size }
            item(key = "method-$sectionIndex") {
                Column(Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(horizontal = 20.dp)) {
                    section.name?.takeIf { it.isNotBlank() }?.let { name ->
                        Text(name.uppercase(), style = CrumbText.kicker, color = c.primary, modifier = Modifier.padding(bottom = 12.dp))
                    }
                    section.items.forEachIndexed { itemIndex, step ->
                        Row(Modifier.fillMaxWidth().padding(vertical = 12.dp), verticalAlignment = Alignment.Top) {
                            Text(
                                "${base + itemIndex + 1}",
                                color = c.primary,
                                fontFamily = DmSerif,
                                fontSize = 28.sp,
                                lineHeight = 28.sp,
                                textAlign = TextAlign.End,
                                modifier = Modifier.width(36.dp),
                            )
                            Text(
                                step,
                                style = CrumbText.body.copy(fontSize = 17.sp, lineHeight = 26.sp),
                                color = c.ink,
                                modifier = Modifier.weight(1f).padding(start = 16.dp),
                            )
                        }
                    }
                }
            }
        }

        recipe.notes?.takeIf { it.isNotBlank() }?.let { notes ->
            item {
                Column(Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 28.dp)) {
                    Card(Modifier.fillMaxWidth(), padding = PaddingValues(24.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Icon(Lucide.StickyNote, contentDescription = null, tint = c.primary, modifier = Modifier.size(18.dp))
                            Text("Notes", color = c.primary, fontFamily = NunitoSans, fontSize = 16.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(start = 8.dp))
                        }
                        Text(
                            notes,
                            style = CrumbText.hand.copy(fontSize = 26.sp, lineHeight = 30.sp),
                            color = c.ink,
                            modifier = Modifier.padding(top = 8.dp),
                        )
                    }
                }
            }
        }

        val nutrition = recipe.nutrition
        if (nutrition != null && nutrition.isNotEmpty()) {
            item {
                Text(
                    "Nutrition",
                    style = CrumbText.sectionTitle,
                    color = c.ink,
                    modifier = Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 28.dp, bottom = 8.dp),
                )
            }
            item {
                Column(Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(horizontal = 20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    nutrition.entries.toList().chunked(2).forEach { row ->
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                            row.forEach { (key, value) ->
                                Card(Modifier.weight(1f), padding = PaddingValues(horizontal = 16.dp, vertical = 12.dp)) {
                                    Text(nutritionLabel(key), style = CrumbText.meta, color = c.inkMuted)
                                    Text(
                                        nutritionValue(value).orEmpty(),
                                        color = c.ink,
                                        fontFamily = NunitoSans,
                                        fontSize = 16.sp,
                                        fontWeight = FontWeight.Bold,
                                        modifier = Modifier.padding(top = 2.dp),
                                    )
                                }
                            }
                            if (row.size == 1) Spacer(Modifier.weight(1f))
                        }
                    }
                }
            }
        }

        if (state.cookbooks.isNotEmpty()) {
            item {
                Text(
                    "On the shelf in",
                    style = CrumbText.sectionTitle,
                    color = c.ink,
                    modifier = Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 28.dp, bottom = 12.dp),
                )
            }
            item {
                FlowRow(
                    Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(horizontal = 20.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    state.cookbooks.forEach { book ->
                        ShelfChip(book, book.id in state.inCookbooks) { vm.toggleCookbook(book.id) }
                    }
                }
            }
        }

        item { Box(Modifier.navigationBarsPadding().padding(bottom = 24.dp)) }
    }

    if (showShare) {
        ShareSheet(
            kind = ShareKind.Recipe,
            id = recipe.id,
            title = recipe.title,
            share = state.share,
            onShareChange = { vm.setShare(it) },
            onDismiss = { showShare = false },
            recipe = recipe,
        )
    }

    if (showDelete) {
        CrumbModal(
            title = "Delete this recipe?",
            description = "This can't be undone.",
            onDismiss = { showDelete = false },
            footer = {
                Btn("Cancel", { showDelete = false }, style = BtnStyle.Ghost)
                Btn(
                    "Delete",
                    {
                        scope.launch {
                            if (vm.delete()) {
                                container.local.forgetViewed(listOf(recipe.id))
                                Toaster.show("Recipe deleted")
                                showDelete = false
                                nav.back()
                            }
                        }
                    },
                    style = BtnStyle.Danger,
                )
            },
        ) {}
    }
}

/** "By Sam · seriouseats.com", with the cooked badge in front when there is one. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Byline(recipe: Recipe, cooked: String?, sourceHost: String?, onOpenSource: (String) -> Unit) {
    val c = Crumb.colors
    val author = recipe.author?.takeIf { it.isNotBlank() }
    val sourceUrl = webLink(recipe.originalUrl) ?: webLink(recipe.url)
    if (cooked == null && author == null && (sourceHost == null || sourceUrl == null)) return
    FlowRow(
        Modifier.fillMaxWidth().padding(top = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        if (cooked != null) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Icon(Lucide.ChefHat, contentDescription = null, tint = c.primary, modifier = Modifier.size(16.dp))
                Text(cooked, color = c.ink, fontFamily = NunitoSans, fontSize = 14.sp, fontWeight = FontWeight.Bold)
            }
        }
        if (author != null) {
            if (cooked != null) Dot()
            Text("By $author", color = c.inkMuted, fontFamily = NunitoSans, fontSize = 14.sp)
        }
        if (sourceUrl != null && sourceHost != null) {
            if (cooked != null || author != null) Dot()
            Row(
                Modifier.heightIn(min = 44.dp).clickable(role = Role.Button) { onOpenSource(sourceUrl) },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(sourceHost, color = c.primary, fontFamily = NunitoSans, fontSize = 14.sp, fontWeight = FontWeight.Bold)
                Icon(Lucide.ExternalLink, contentDescription = null, tint = c.primary, modifier = Modifier.size(14.dp))
            }
        }
    }
}

@Composable
private fun Dot() {
    Text("·", color = Crumb.colors.inkMuted, fontFamily = NunitoSans, fontSize = 14.sp, modifier = Modifier.padding(horizontal = 2.dp))
}

private data class TimesEntry(val label: String, val icon: ImageVector, val value: String)

/** The times and yield, three to a row, only the fields that have a value (spec §4.6). */
@Composable
private fun TimesCard(entries: List<TimesEntry>, modifier: Modifier = Modifier) {
    val c = Crumb.colors
    Card(modifier.fillMaxWidth()) {
        entries.chunked(3).forEach { row ->
            Row(Modifier.fillMaxWidth()) {
                row.forEach { entry ->
                    Column(Modifier.weight(1f).padding(horizontal = 14.dp, vertical = 12.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            Icon(entry.icon, contentDescription = null, tint = c.primary, modifier = Modifier.size(16.dp))
                            Text(entry.label, style = CrumbText.meta, color = c.inkMuted)
                        }
                        Text(
                            entry.value,
                            color = c.ink,
                            fontFamily = NunitoSans,
                            fontSize = 17.sp,
                            fontWeight = FontWeight.Bold,
                            modifier = Modifier.padding(top = 4.dp),
                        )
                    }
                }
                repeat(3 - row.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

/** One ingredient line with its round tick (web's `.ingredients` row). */
@Composable
private fun IngredientRow(line: String, done: Boolean, scale: Double, onToggle: () -> Unit) {
    val c = Crumb.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .clickable(role = Role.Checkbox, onClick = onToggle)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.Top,
    ) {
        Box(
            Modifier
                .padding(top = 1.dp)
                .size(24.dp)
                .clip(CircleShape)
                .background(if (done) c.tile else Color.Transparent)
                .border(2.dp, if (done) c.tile else c.lineStrong, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            if (done) Icon(Lucide.Check, contentDescription = null, tint = c.onTile, modifier = Modifier.size(14.dp))
        }
        Text(
            scaleIngredient(line, scale),
            style = CrumbText.body.copy(lineHeight = 22.sp),
            color = if (done) c.inkMuted else c.ink,
            textDecoration = if (done) TextDecoration.LineThrough else null,
            modifier = Modifier.weight(1f).padding(start = 12.dp),
        )
    }
}

/** A cookbook chip with its little spine (web's `.chip` under "On the shelf in"). */
@Composable
private fun ShelfChip(book: CookbookListItem, inBook: Boolean, onToggle: () -> Unit) {
    val c = Crumb.colors
    val spine = RoundedCornerShape(topStart = 2.dp, topEnd = 2.dp)
    Row(
        Modifier
            .height(44.dp)
            .clip(CircleShape)
            .background(if (inBook) c.tint else c.paper)
            .border(1.dp, if (inBook) Color.Transparent else c.line, CircleShape)
            .clickable(role = Role.Checkbox, onClick = onToggle)
            .padding(start = 10.dp, end = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Box(
            Modifier
                .size(width = 10.dp, height = 16.dp)
                .clip(spine)
                .background(bookLook(book.color).cloth)
                .border(1.dp, c.ink.copy(alpha = 0.22f), spine),
        )
        Text(book.name, color = if (inBook) c.primary else c.inkMuted, fontFamily = NunitoSans, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1)
        Icon(if (inBook) Lucide.Check else Lucide.Plus, contentDescription = null, tint = if (inBook) c.primary else c.inkMuted, modifier = Modifier.size(16.dp))
    }
}

/** The ⋯ menu's groups (spec §4.13). */
private fun recipeMenu(
    state: RecipePageState,
    vm: RecipePageViewModel,
    onEdit: () -> Unit,
    onSurprise: () -> Unit,
    onDelete: () -> Unit,
): List<List<MenuItem>> = buildList {
    add(
        listOf(
            MenuItem("Mark as cooked", Lucide.ChefHat) { vm.markCooked() },
            MenuItem("Edit", Lucide.Pencil, onSelect = onEdit),
            MenuItem("Surprise me", Lucide.Dices, onSelect = onSurprise),
        ),
    )
    if (state.weeChefChecks) add(listOf(MenuItem("Check with Wee Chef", Lucide.ClipboardCheck) { vm.checkNow() }))
    add(listOf(MenuItem("Delete", Lucide.Trash2, danger = true, onSelect = onDelete)))
}

private fun openUrl(context: Context, url: String) {
    runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, url.toUri())) }
}

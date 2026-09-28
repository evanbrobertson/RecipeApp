package app.crumb.android.ui.suggestions

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import app.crumb.android.data.ApiException
import app.crumb.android.data.ChecksStatus
import app.crumb.android.data.CrumbApi
import app.crumb.android.data.ReviewRecipe
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnSize
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.Card
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.EmptyState
import app.crumb.android.ui.components.ListCard
import app.crumb.android.ui.components.Message
import app.crumb.android.ui.components.ProgressBar
import app.crumb.android.ui.components.RecipePhoto
import app.crumb.android.ui.components.Skeleton
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.components.VSpace
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import com.composables.icons.lucide.ChefHat
import com.composables.icons.lucide.ChevronRight
import com.composables.icons.lucide.ClipboardCheck
import com.composables.icons.lucide.LoaderCircle
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Sparkles
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlin.math.max

private const val PhotoWidthPx = 160

/** Everything the Suggestions page shows: the review list and Wee Chef's check progress. */
data class SuggestionsUiState(
    /** null until the first load lands; empty means nothing is waiting. */
    val recipes: List<ReviewRecipe>? = null,
    /** Only set when Wee Chef checks are on (web keeps `checks` undefined otherwise). */
    val checks: ChecksStatus? = null,
    val starting: Boolean = false,
    /** The most this Check all had pending at once, so progress counts failures as done. */
    val run: Int = 0,
    val error: String? = null,
    val signedOut: Boolean = false,
)

/**
 * The Suggestions page's state (web/src/islands/SuggestionsPage.svelte). Loads the review list
 * and Wee Chef's check status together, then, while anything is pending, polls with the web's
 * backoff (1.5s after a recipe finishes, ×1.5 up to 8s, ×2 up to 10s after a failure) and
 * reloads the list whenever the counts move. The nav badge follows the list length.
 */
class SuggestionsViewModel(private val api: CrumbApi) : ViewModel() {
    private val _state = MutableStateFlow(SuggestionsUiState())
    val state: StateFlow<SuggestionsUiState> = _state.asStateFlow()

    private var loadJob: Job? = null
    private var pollJob: Job? = null
    private var shown = false

    init {
        load()
    }

    /** Back on screen: reload unless this is the first time (init already did). */
    fun resumed() {
        if (shown) load() else shown = true
    }

    fun load() {
        loadJob?.cancel()
        _state.update { it.copy(recipes = null, error = null) }
        loadJob = viewModelScope.launch {
            try {
                val list = api.reviewList()
                // Checks may be off for this box; the card then stays hidden
                val status = runCatching { api.checksStatus() }.getOrNull()
                applyList(list)
                applyChecks(status?.takeIf { it.enabled })
                if ((_state.value.checks?.pending ?: 0) > 0) watch(PollStartMs)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _state.update {
                    it.copy(recipes = null, error = e.friendlyMessage(), signedOut = (e as? ApiException)?.isSignedOut == true)
                }
            }
        }
    }

    /** "Check all with Wee Chef" (web `checkAll`). */
    fun checkAll() {
        val checks = _state.value.checks ?: return
        if (_state.value.starting || checks.pending > 0 || checks.due <= 0) return
        _state.update { it.copy(starting = true) }
        viewModelScope.launch {
            try {
                val next = api.checkAll()
                applyChecks(next)
                if (next.queued <= 0) {
                    Toaster.show("Every recipe has been checked")
                } else {
                    watch(PollStartMs)
                }
            } catch (e: Exception) {
                Toaster.show("Couldn't start the checks", e.friendlyMessage(), ToastTone.Error)
            } finally {
                _state.update { it.copy(starting = false) }
            }
        }
    }

    private fun applyList(list: List<ReviewRecipe>) {
        _state.update { it.copy(recipes = list, error = null, signedOut = false) }
        ReviewCount.set(list.size)
    }

    private fun applyChecks(status: ChecksStatus?) {
        if (status == null) return
        _state.update { s ->
            s.copy(checks = status, run = if (status.pending == 0) 0 else max(s.run, status.pending))
        }
    }

    private suspend fun refreshList() {
        runCatching { api.reviewList() }.getOrNull()?.let(::applyList)
    }

    private fun watch(initialDelay: Long) {
        if (pollJob?.isActive == true) return
        pollJob = viewModelScope.launch {
            var delayMs = initialDelay
            while (true) {
                delay(delayMs)
                val before = _state.value.checks
                val next = runCatching { api.checksStatus() }.getOrNull()
                if (next == null) {
                    delayMs = nextPollDelay(delayMs, moved = false, failed = true)
                } else {
                    val moved = next.checked != before?.checked || next.toCheck != before?.toCheck
                    applyChecks(next)
                    if (moved || next.pending == 0) refreshList()
                    delayMs = nextPollDelay(delayMs, moved = moved, failed = false)
                }
                if ((_state.value.checks?.pending ?: 0) <= 0) break
            }
        }
    }
}

/**
 * Suggestions / Review (web/src/islands/SuggestionsPage.svelte): Wee Chef's check status card,
 * the recipes with flags waiting, and a badge on the Review tab.
 */
@Composable
fun SuggestionsScreen() {
    val container = AppContainerProvider
    val nav = LocalNav.current
    val vm: SuggestionsViewModel = crumbViewModel { SuggestionsViewModel(container.api) }
    val state by vm.state.collectAsStateWithLifecycle()

    LifecycleResumeEffect(Unit) {
        vm.resumed()
        onPauseOrDispose {}
    }

    LaunchedEffect(state.signedOut) {
        if (state.signedOut) nav.signedOut()
    }

    Box(Modifier.fillMaxSize().statusBarsPadding()) {
        when {
            state.error != null && state.recipes == null -> Message(
                "Couldn't load suggestions",
                state.error,
                action = { Btn("Try again", { vm.load() }, style = BtnStyle.Outline) },
            )
            state.recipes == null -> SuggestionsSkeleton()
            else -> SuggestionsContent(state, onCheckAll = vm::checkAll, photoUrl = { id, image ->
                container.recipes.photoUrl(id, image, PhotoWidthPx)
            }, onEdit = nav::edit)
        }
    }
}

@Composable
private fun SuggestionsContent(
    state: SuggestionsUiState,
    onCheckAll: () -> Unit,
    photoUrl: (Long, String?) -> String?,
    onEdit: (Long) -> Unit,
) {
    val recipes = state.recipes.orEmpty()
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 16.dp, bottom = 28.dp),
        verticalArrangement = Arrangement.spacedBy(24.dp),
    ) {
        item { Header() }
        state.checks?.let { checks -> item { ChecksCard(checks, state, onCheckAll) } }
        if (recipes.isEmpty()) {
            item {
                EmptyState(
                    icon = Lucide.Sparkles,
                    title = "Nothing to review",
                    description = if (state.checks != null) {
                        "Wee Chef will flag anything it isn't sure about."
                    } else {
                        "Wee Chef checks aren't on for this box."
                    },
                )
            }
        } else {
            item {
                val rows = mutableListOf<@Composable () -> Unit>()
                recipes.forEach { recipe ->
                    rows += { ReviewRow(recipe, photoUrl(recipe.id, recipe.image), onEdit) }
                }
                ListCard(rows = rows)
            }
        }
    }
}

@Composable
private fun Header() {
    val c = Crumb.colors
    Column {
        Text("Suggestions", style = CrumbText.pageTitle, color = c.ink)
        VSpace(4.dp)
        Text(
            "Wee Chef reads your recipes and flags anything worth a look. Nothing changes until you say so.",
            style = CrumbText.body,
            color = c.inkMuted,
        )
    }
}

/** The "Wee Chef checks" card (web SuggestionsPage §9.2): status, progress while running, button. */
@Composable
private fun ChecksCard(checks: ChecksStatus, state: SuggestionsUiState, onCheckAll: () -> Unit) {
    val c = Crumb.colors
    val running = checks.pending > 0
    Card(Modifier.fillMaxWidth(), padding = PaddingValues(20.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Box(
                Modifier.size(48.dp).clip(ControlShape).background(c.tint),
                contentAlignment = Alignment.Center,
            ) {
                Icon(Lucide.ChefHat, contentDescription = null, tint = c.primary, modifier = Modifier.size(24.dp))
            }
            Column(Modifier.weight(1f)) {
                Text("Wee Chef checks", style = CrumbText.body.copy(fontWeight = FontWeight.Bold), color = c.ink)
                Text(
                    checksStatusText(checks, state.run),
                    style = CrumbText.bodySmall,
                    color = c.inkMuted,
                    modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
                )
                if (running) {
                    VSpace(8.dp)
                    ProgressBar(checksProgress(state.run, checks.pending))
                }
            }
        }
        VSpace(16.dp)
        CheckAllButton(
            busy = running || state.starting,
            enabled = !running && !state.starting && checks.due > 0,
            onClick = onCheckAll,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/**
 * Butter, the one main action on this screen. The kit's [Btn] can't spin its icon, so the
 * loader turns here to mirror the web's `animate-spin` LoaderCircle.
 */
@Composable
private fun CheckAllButton(busy: Boolean, enabled: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val c = Crumb.colors
    val spin = rememberInfiniteTransition(label = "check-all")
    val angle by spin.animateFloat(0f, 360f, infiniteRepeatable(tween(900, easing = LinearEasing)), label = "angle")
    val ink = if (enabled) c.onButter else c.inkMuted
    Row(
        modifier
            .height(BtnSize.Md.height)
            .clip(ControlShape)
            .background(if (enabled) c.butter else c.tint)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(horizontal = BtnSize.Md.padding),
        horizontalArrangement = Arrangement.spacedBy(BtnSize.Md.gap, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            if (busy) Lucide.LoaderCircle else Lucide.ClipboardCheck,
            contentDescription = null,
            tint = ink,
            modifier = Modifier
                .size(BtnSize.Md.icon)
                .then(if (busy) Modifier.rotate(angle) else Modifier),
        )
        Text(
            if (busy) "Checking…" else "Check all with Wee Chef",
            color = ink,
            fontFamily = NunitoSans,
            fontSize = 15.sp,
            fontWeight = FontWeight.Bold,
            maxLines = 1,
        )
    }
}

/** One waiting recipe (web `.list-row`): photo, title, `summary(fields)`, count, chevron. */
@Composable
private fun ReviewRow(recipe: ReviewRecipe, photoUrl: String?, onEdit: (Long) -> Unit) {
    val c = Crumb.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 68.dp)
            .clickable(role = Role.Button) { onEdit(recipe.id) }
            .padding(12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        RecipePhoto(
            url = photoUrl,
            image = recipe.image,
            modifier = Modifier.size(56.dp),
            placeholderId = recipe.id,
        )
        Column(Modifier.weight(1f)) {
            Text(recipe.title, style = CrumbText.rowTitle, color = c.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(summary(recipe.fields), style = CrumbText.bodySmall, color = c.inkMuted, maxLines = 1)
        }
        CountBadge(recipe.count)
        Icon(Lucide.ChevronRight, contentDescription = null, tint = c.inkMuted, modifier = Modifier.size(20.dp))
    }
}

/** The web's `.review-count`: a tint pill with the primary count (13sp, extra bold). */
@Composable
private fun CountBadge(count: Int) {
    val c = Crumb.colors
    Box(
        Modifier
            .defaultMinSize(minWidth = 28.dp, minHeight = 28.dp)
            .clip(RoundedCornerShape(50))
            .background(c.tint)
            .padding(horizontal = 8.dp)
            .semantics { contentDescription = "$count to check" },
        contentAlignment = Alignment.Center,
    ) {
        Text(
            count.toString(),
            color = c.primary,
            fontFamily = NunitoSans,
            fontSize = 13.sp,
            fontWeight = FontWeight.ExtraBold,
            maxLines = 1,
        )
    }
}

/** The web's fallback while the list loads: a short card and a tall one. */
@Composable
private fun SuggestionsSkeleton() {
    Column(Modifier.fillMaxSize().padding(horizontal = 20.dp, vertical = 16.dp)) {
        Skeleton(Modifier.fillMaxWidth().height(112.dp))
        VSpace(24.dp)
        Skeleton(Modifier.fillMaxWidth().height(192.dp))
    }
}

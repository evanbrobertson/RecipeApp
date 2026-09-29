package app.crumb.android.ui.preview

import android.content.Context
import android.content.Intent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
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
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.net.toUri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import app.crumb.android.data.CrumbApi
import app.crumb.android.data.ImportOutcome
import app.crumb.android.data.Recipe
import app.crumb.android.data.SITE_TERMS
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnSize
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.Message
import app.crumb.android.ui.components.RecipePhoto
import app.crumb.android.ui.components.Skeleton
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.home.showOutcome
import app.crumb.android.ui.recipe.Byline
import app.crumb.android.ui.recipe.RecipeVideoCard
import app.crumb.android.ui.recipe.RecipeVideoPlayer
import app.crumb.android.ui.recipe.TimesCard
import app.crumb.android.ui.recipe.VideoState
import app.crumb.android.ui.recipe.ingredientItems
import app.crumb.android.ui.recipe.methodItems
import app.crumb.android.ui.recipe.notesItem
import app.crumb.android.ui.recipe.recipeTimes
import app.crumb.android.ui.recipe.videoEmbedFor
import app.crumb.android.ui.theme.Crumb
import app.crumb.core.hostOf
import app.crumb.core.kicker
import app.crumb.core.scaleIngredient
import app.crumb.core.webLink
import com.composables.icons.lucide.LoaderCircle
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Plus
import com.composables.icons.lucide.RotateCw
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** Reads a link with the server, without saving it. */
class PreviewViewModel(private val api: CrumbApi, private val url: String) : ViewModel() {
    private val _state = MutableStateFlow<PreviewState>(PreviewState.Reading)
    val state: StateFlow<PreviewState> = _state.asStateFlow()

    init {
        load()
    }

    fun load() {
        _state.value = PreviewState.Reading
        viewModelScope.launch {
            _state.value = try {
                stateOf(api.preview(url))
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                failedState(e, e.friendlyMessage())
            }
        }
    }
}

/**
 * A recipe link read and shown before it's saved (web/src/pages/shell/preview): opened by
 * Share → Crumb with one link, and by a Popular link. It reads like the recipe page, and
 * "Add to my Crumb" saves it (the server reuses the reading it just made) and opens it.
 */
@Composable
fun PreviewScreen(url: String) {
    val container = AppContainerProvider
    val nav = LocalNav.current
    val scope = rememberCoroutineScope()
    val vm = crumbViewModel(key = "preview-$url") { PreviewViewModel(it.api, url) }
    val state by vm.state.collectAsStateWithLifecycle()
    var adding by remember { mutableStateOf(false) }
    var line by remember { mutableStateOf("") }
    // A video (or a shared cookbook) skips the reading: it is saved as the Add box saves it
    var importing by remember { mutableStateOf(false) }
    var importError by remember { mutableStateOf<PreviewState.Failed?>(null) }

    /** Saves the link, then leaves the preview for what was saved, so Back doesn't return here. */
    fun save(onFailure: (Throwable) -> Unit) {
        scope.launch {
            try {
                val outcome = container.importer.link(url) { line = it }
                if (outcome is ImportOutcome.Saved && outcome.isNew && !outcome.fromVideo && !outcome.droppedPhoto && !outcome.onDevice) {
                    Toaster.show("Added to your box", tone = ToastTone.Success)
                }
                nav.back()
                showOutcome(outcome, nav)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                onFailure(e)
            }
        }
    }

    LaunchedEffect(state) {
        when (val s = state) {
            is PreviewState.Saved -> {
                Toaster.show("Already in your recipes")
                nav.replaceWithRecipe(s.id)
            }
            PreviewState.Import -> if (!importing) {
                importing = true
                save { e ->
                    importing = false
                    importError = failedState(e, e.friendlyMessage())
                }
            }
            is PreviewState.Failed -> if (s.signedOut) nav.signedOut()
            else -> Unit
        }
    }

    val failed = importError ?: state as? PreviewState.Failed
    Box(Modifier.fillMaxSize()) {
        when {
            failed != null -> Message(
                if (failed.code == SITE_TERMS) "This site doesn't allow it" else "Couldn't read that recipe",
                failed.message,
                action = {
                    Column(verticalArrangement = Arrangement.spacedBy(12.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                        if (canTryAgain(failed.code)) {
                            Btn("Try again", { importError = null; vm.load() }, style = BtnStyle.Primary, icon = Lucide.RotateCw)
                        }
                        Btn("Paste the recipe text instead", { nav.back(); nav.add(textMode = true) }, style = BtnStyle.Soft)
                    }
                },
            )
            state is PreviewState.Ready -> {
                val recipe = (state as PreviewState.Ready).recipe
                PreviewContent(
                    recipe = recipe,
                    url = url,
                    adding = adding,
                    line = line,
                    onAdd = {
                        if (!adding) {
                            adding = true
                            line = ""
                            save { e ->
                                adding = false
                                line = ""
                                Toaster.show("Couldn't add it", e.friendlyMessage(), ToastTone.Error)
                            }
                        }
                    },
                    onNotNow = { nav.back() },
                )
            }
            else -> Waiting(url, if (importing) line.ifEmpty { "Adding it to your Crumb…" } else null)
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

/** "Reading the recipe…" over the shapes of a recipe, or a video's progress line while it is saved. */
@Composable
private fun Waiting(url: String, savingLine: String?) {
    val c = Crumb.colors
    Column(
        Modifier.fillMaxSize().statusBarsPadding().padding(start = 20.dp, end = 20.dp, top = 72.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Column {
            Text(fromHost(url).uppercase(), style = CrumbText.kicker, color = c.primary, modifier = Modifier.padding(bottom = 8.dp))
            Text(if (savingLine != null) "Adding it to your Crumb…" else "Reading the recipe…", style = CrumbText.pageTitle, color = c.ink)
            if (savingLine != null) Text(savingLine, style = CrumbText.body, color = c.inkMuted, modifier = Modifier.padding(top = 8.dp))
        }
        Skeleton(Modifier.fillMaxWidth().aspectRatio(4f / 3f))
        repeat(6) { Skeleton(Modifier.fillMaxWidth().height(20.dp)) }
    }
}

@Composable
private fun PreviewContent(
    recipe: Recipe,
    url: String,
    adding: Boolean,
    line: String,
    onAdd: () -> Unit,
    onNotNow: () -> Unit,
) {
    val c = Crumb.colors
    val context = LocalContext.current
    var scale by rememberSaveable { mutableStateOf(1.0) }
    var ticked by rememberSaveable { mutableStateOf(setOf<String>()) }
    // The photo is the site's own, not a copy on this server, so it is loaded from there
    val photo = recipe.image?.takeIf { it.startsWith("https://") || it.startsWith("http://") }
    val sub = kicker(recipe.recipeCategory, recipe.recipeCuisine).ifBlank { fromHost(recipe.url ?: url) }
    val sourceUrl = webLink(recipe.url ?: url)
    val sourceHost = hostOf(recipe.url ?: url)
    val scaledYield = recipe.recipeYield?.takeIf { it.isNotBlank() }?.let { if (scale == 1.0) it else scaleIngredient(it, scale) }
    val times = recipeTimes(recipe, scaledYield)
    val hasIngredients = recipe.ingredients.any { it.items.isNotEmpty() }

    val listState = rememberLazyListState()
    val video = recipe.video?.takeIf { it.isNotBlank() }
    val embed = remember(recipe.video, recipe.videoEmbed) { videoEmbedFor(recipe) }
    val videoState = remember(url) { VideoState() }
    var origin by remember { mutableStateOf(Offset.Zero) }
    // After the photo, the header and the ingredients title (plus the "none listed" line, or a card per section)
    val videoIndex = 3 + (if (hasIngredients) 0 else 1) + recipe.ingredients.count { it.items.isNotEmpty() }

    Box(Modifier.fillMaxSize()) {
        LazyColumn(
            Modifier.fillMaxSize().testTag("preview").onGloballyPositioned { origin = it.positionInRoot() },
            state = listState,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            item {
                if (photo != null) {
                    Box(Modifier.fillMaxWidth().statusBarsPadding().padding(horizontal = 16.dp, vertical = 8.dp)) {
                        RecipePhoto(photo, recipe.image, Modifier.fillMaxWidth().aspectRatio(4f / 3f), contentDescription = recipe.title)
                    }
                } else {
                    Box(Modifier.statusBarsPadding().padding(top = 56.dp))
                }
            }
            item {
                Column(Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 16.dp)) {
                    Text(sub.uppercase(), style = CrumbText.kicker, color = c.primary, modifier = Modifier.padding(bottom = 8.dp))
                    Text(recipe.title, style = CrumbText.pageTitle, color = c.ink)
                    Byline(recipe, cooked = null, sourceHost = sourceHost) { link -> openLink(context, link) }
                    recipe.description?.takeIf { it.isNotBlank() }?.let { description ->
                        Text(
                            description,
                            style = CrumbText.body.copy(fontSize = 17.sp, lineHeight = 26.sp),
                            color = c.inkMuted,
                            modifier = Modifier.padding(top = 16.dp),
                        )
                    }
                    if (times.isNotEmpty()) TimesCard(times, Modifier.padding(top = 16.dp))
                }
            }
            ingredientItems(
                recipe, scale, onScale = { scale = it }, ticked = ticked,
                onToggle = { key -> ticked = if (key in ticked) ticked - key else ticked + key },
            )
            if (video != null) {
                item(key = "video") {
                    RecipeVideoCard(
                        video, embed, recipe.title, videoState,
                        Modifier.widthIn(max = 720.dp).fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 28.dp),
                    )
                }
            }
            methodItems(recipe)
            recipe.notes?.takeIf { it.isNotBlank() }?.let { notesItem(it) }
            // Room for the bar below
            item { Box(Modifier.navigationBarsPadding().height(190.dp)) }
        }
        if (video != null && embed != null) {
            RecipeVideoPlayer(videoState, embed, recipe.title, listState, videoIndex, "video", origin)
        }
        Column(
            Modifier
                .align(Alignment.BottomCenter)
                .fillMaxWidth()
                .background(c.canvas)
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            HorizontalDivider(color = c.line, modifier = Modifier.padding(bottom = 8.dp))
            Btn(
                if (adding) "Adding…" else "Add to my Crumb",
                onAdd,
                style = BtnStyle.Primary,
                size = BtnSize.Xl,
                icon = if (adding) Lucide.LoaderCircle else Lucide.Plus,
                enabled = !adding,
                modifier = Modifier.fillMaxWidth().testTag("add-to-crumb"),
            )
            Text(
                line.ifEmpty { "Not in your box yet." },
                style = CrumbText.bodySmall,
                color = c.inkMuted,
                modifier = Modifier.padding(start = 4.dp, top = 4.dp),
            )
            Btn("Not now", onNotNow, style = BtnStyle.Ghost, enabled = !adding, modifier = Modifier.fillMaxWidth())
        }
    }
}

private fun openLink(context: Context, url: String) {
    runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, url.toUri())) }
}

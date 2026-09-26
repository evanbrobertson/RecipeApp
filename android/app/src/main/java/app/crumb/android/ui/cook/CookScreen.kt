package app.crumb.android.ui.cook

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.ArrowForward
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.automirrored.outlined.FormatListBulleted
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import app.crumb.android.timers.rememberTimerStarter
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnSize
import app.crumb.android.ui.components.BtnStyle
import app.crumb.core.findTimers
import com.composables.icons.lucide.AlarmClock
import com.composables.icons.lucide.Lucide
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.crumb.android.data.Recipe
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.UiState
import app.crumb.android.ui.components.Loading
import app.crumb.android.ui.components.Message
import app.crumb.android.ui.components.PrimaryButton
import app.crumb.android.ui.components.SecondaryButton
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.recipe.recipeViewModel
import app.crumb.android.ui.theme.Crumb
import kotlinx.coroutines.launch

/** One instruction, with the section it belongs to ("For the sauce"). */
data class CookStep(val section: String?, val text: String)

fun cookSteps(recipe: Recipe): List<CookStep> = recipe.instructions.flatMap { section ->
    section.items.filter { it.isNotBlank() }.map { CookStep(section.name?.takeIf { n -> n.isNotBlank() }, it) }
}

/** Cook mode: one step per page, big type, screen stays on. */
@Composable
fun CookScreen(id: Long, onClose: () -> Unit) {
    val vm = recipeViewModel(id)
    val state by vm.state.collectAsStateWithLifecycle()

    // Keep the screen awake while cooking
    val view = LocalView.current
    DisposableEffect(view) {
        view.keepScreenOn = true
        onDispose { view.keepScreenOn = false }
    }

    when (val s = state) {
        UiState.Loading -> Loading()
        is UiState.Failed -> Message(
            "Couldn't open this recipe",
            s.message,
            action = { SecondaryButton("Try again", onClick = { vm.load() }) },
        )
        is UiState.Ready -> CookContent(s.value, onClose)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CookContent(recipe: Recipe, onClose: () -> Unit) {
    val colors = Crumb.colors
    val steps = remember(recipe) { cookSteps(recipe) }
    val startTimer = rememberTimerStarter()
    val container = AppContainerProvider
    // Page = steps.size is the "all done" page
    var savedPage by rememberSaveable(recipe.id) { mutableStateOf(0) }
    val pager = rememberPagerState(initialPage = savedPage) { steps.size + 1 }
    var showIngredients by rememberSaveable { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val snackbar = remember { SnackbarHostState() }
    var logged by rememberSaveable(recipe.id) { mutableStateOf(false) }

    LaunchedEffect(pager.currentPage) { savedPage = pager.currentPage }

    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding()) {
            Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onClose, modifier = Modifier.testTag("close-cook")) {
                    Icon(Icons.Outlined.Close, contentDescription = "Stop cooking")
                }
                Text(
                    recipe.title,
                    style = MaterialTheme.typography.titleSmall,
                    maxLines = 1,
                    modifier = Modifier.weight(1f),
                )
                IconButton(onClick = { showIngredients = true }) {
                    Icon(Icons.AutoMirrored.Outlined.FormatListBulleted, contentDescription = "Ingredients")
                }
            }
            LinearProgressIndicator(
                progress = { if (steps.isEmpty()) 1f else (pager.currentPage + 1).coerceAtMost(steps.size).toFloat() / steps.size },
                color = colors.primary,
                trackColor = colors.tint,
                drawStopIndicator = {},
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
            )
            HorizontalPager(pager, modifier = Modifier.weight(1f).testTag("cook-pager")) { page ->
                Column(
                    Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(24.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.Center,
                ) {
                    Column(Modifier.widthIn(max = 640.dp).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                        if (page < steps.size) {
                            val step = steps[page]
                            Text(
                                listOfNotNull("Step ${page + 1} of ${steps.size}", step.section).joinToString(" · "),
                                style = MaterialTheme.typography.labelLarge,
                                color = colors.primary,
                            )
                            Text(step.text, style = MaterialTheme.typography.headlineSmall.copy(lineHeight = MaterialTheme.typography.headlineSmall.lineHeight))
                            // One button per duration in the step ("Bake for 25 minutes")
                            findTimers(step.text).forEach { t ->
                                Btn(
                                    "Start ${t.label} timer",
                                    { startTimer("Step ${page + 1}: ${t.label}", t.seconds.toInt()) },
                                    style = BtnStyle.Tile, size = BtnSize.Lg, icon = Lucide.AlarmClock,
                                    modifier = Modifier.padding(top = 16.dp),
                                )
                            }
                        } else {
                            Text("All done", style = MaterialTheme.typography.headlineLarge)
                            Text("Enjoy it. Want to note that you cooked this?", style = MaterialTheme.typography.bodyLarge, color = colors.inkMuted)
                            PrimaryButton(
                                if (logged) "Logged" else "I cooked this",
                                enabled = !logged,
                                icon = Icons.Outlined.Check,
                                onClick = {
                                    scope.launch {
                                        try {
                                            container.recipes.markCooked(recipe.id)
                                            logged = true
                                        } catch (e: Exception) {
                                            snackbar.showSnackbar(e.friendlyMessage())
                                        }
                                    }
                                },
                                modifier = Modifier.fillMaxWidth().testTag("cooked"),
                            )
                        }
                    }
                }
            }
            Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                SecondaryButton(
                    "Back",
                    icon = Icons.AutoMirrored.Outlined.ArrowBack,
                    onClick = { scope.launch { pager.animateScrollToPage(pager.currentPage - 1) } },
                    modifier = Modifier.weight(1f),
                )
                if (pager.currentPage < steps.size) {
                    PrimaryButton(
                        if (pager.currentPage == steps.size - 1) "Finish" else "Next",
                        icon = Icons.AutoMirrored.Outlined.ArrowForward,
                        onClick = { scope.launch { pager.animateScrollToPage(pager.currentPage + 1) } },
                        modifier = Modifier.weight(1f).testTag("next-step"),
                    )
                } else {
                    PrimaryButton("Close", onClick = onClose, modifier = Modifier.weight(1f))
                }
            }
        }
        SnackbarHost(snackbar, Modifier.align(Alignment.BottomCenter).navigationBarsPadding())
    }

    if (showIngredients) {
        ModalBottomSheet(onDismissRequest = { showIngredients = false }, containerColor = colors.paper) {
            LazyColumn(Modifier.fillMaxWidth().padding(horizontal = 20.dp).padding(bottom = 24.dp)) {
                item { Text("Ingredients", style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(bottom = 8.dp)) }
                recipe.ingredients.forEach { section ->
                    section.name?.takeIf { it.isNotBlank() }?.let { name ->
                        item { Text(name, style = MaterialTheme.typography.titleSmall, color = colors.primary, modifier = Modifier.padding(top = 10.dp)) }
                    }
                    items(section.items) { Text("•  $it", style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(vertical = 4.dp)) }
                }
            }
        }
    }
}

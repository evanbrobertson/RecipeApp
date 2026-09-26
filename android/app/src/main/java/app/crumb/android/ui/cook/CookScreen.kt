package app.crumb.android.ui.cook

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.crumb.android.data.CrumbJson
import app.crumb.android.data.Recipe
import app.crumb.android.data.RecipeSession
import app.crumb.android.timers.rememberTimerStarter
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.UiState
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnSize
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.Card
import app.crumb.android.ui.components.CardShape
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.Loading
import app.crumb.android.ui.components.Message
import app.crumb.android.ui.components.Toast
import app.crumb.android.ui.components.ToastAction
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.components.tileSurface
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.recipe.ScaleControl
import app.crumb.android.ui.recipe.recipeViewModel
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.DmSerif
import app.crumb.android.ui.theme.NunitoSans
import app.crumb.core.CookStep
import app.crumb.core.IngredientLine
import app.crumb.core.cookSteps
import app.crumb.core.findTimers
import app.crumb.core.ingredientsForStep
import app.crumb.core.scaleIngredient
import com.composables.icons.lucide.AlarmClock
import com.composables.icons.lucide.ArrowLeft
import com.composables.icons.lucide.ArrowRight
import com.composables.icons.lucide.Check
import com.composables.icons.lucide.ChefHat
import com.composables.icons.lucide.List
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.PartyPopper
import com.composables.icons.lucide.Sun
import com.composables.icons.lucide.X
import kotlinx.coroutines.launch

/**
 * Cook mode (web/src/islands/CookPage.svelte, spec §5): one big step per page on the tile
 * chrome, screen kept on, swipe or tap to move, one-tap timers, the ingredients at hand.
 */
@Composable
fun CookScreen(id: Long, onClose: () -> Unit) {
    val vm = recipeViewModel(id)
    val state by vm.state.collectAsStateWithLifecycle()

    // Android has no wake-lock gesture: keep the screen on for the whole of cook mode
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
            action = { Btn("Try again", { vm.load() }, style = BtnStyle.Outline) },
        )
        is UiState.Ready -> CookContent(s.value, onClose)
    }
}

@Composable
private fun CookContent(recipe: Recipe, onClose: () -> Unit) {
    val c = Crumb.colors
    val container = AppContainerProvider
    val scope = rememberCoroutineScope()
    val startTimer = rememberTimerStarter()

    // Steps and ingredient lines come from crumb-core, from the recipe the repository loaded
    val steps = remember(recipe) {
        runCatching { cookSteps(CrumbJson.encodeToString(Recipe.serializer(), recipe)) }.getOrElse { emptyList() }
    }
    val ingredients = remember(recipe) {
        recipe.ingredients.flatMap { section -> section.items.map { IngredientLine(it, section.name) } }
    }

    var index by remember(recipe.id) { mutableIntStateOf(cookStartIndex(RecipeSession.cookStep(recipe.id), steps.size)) }
    var finished by remember(recipe.id) { mutableStateOf(false) }
    var direction by remember(recipe.id) { mutableIntStateOf(1) }
    var logged by remember(recipe.id) { mutableStateOf(false) }
    var showIngredients by remember { mutableStateOf(false) }
    val checked = remember(recipe.id) { mutableStateOf(emptySet<Int>()) }
    val scale = RecipeSession.scale(recipe.id)

    LaunchedEffect(index) { RecipeSession.setCookStep(recipe.id, index) }

    // markCooked (lib/history.ts): once per visit, with the web's toasts and Undo
    fun markCooked() {
        if (logged) return
        logged = true
        scope.launch {
            try {
                val cooked = container.api.markCooked(recipe.id)
                val eventId = cooked.eventId
                if (eventId == null) {
                    Toaster.show("Already marked as cooked", "Logged in the last few hours.")
                } else {
                    Toaster.show(
                        Toast(
                            title = "Marked as cooked",
                            description = "It'll sit out of Try next for a couple of weeks.",
                            tone = ToastTone.Success,
                            action = ToastAction("Undo") {
                                scope.launch {
                                    runCatching { container.api.undoCooked(recipe.id, eventId) }
                                        .onFailure { Toaster.show("Couldn't undo", it.friendlyMessage(), ToastTone.Error) }
                                }
                            },
                        ),
                    )
                }
            } catch (e: Exception) {
                Toaster.show("Couldn't mark as cooked", e.friendlyMessage(), ToastTone.Error)
            }
        }
    }

    fun go(to: Int) {
        if (to < 0) return
        if (to >= steps.size) {
            finished = true
            markCooked()
            return
        }
        direction = if (to > index) 1 else -1
        finished = false
        index = to
    }

    fun next() = go(index + 1)
    fun prev() {
        if (finished) finished = false else go(index - 1)
    }

    val status = when {
        finished -> "All done"
        steps.isNotEmpty() -> "Step ${index + 1} of ${steps.size}"
        else -> null
    }

    Column(Modifier.fillMaxSize().background(c.canvas)) {
        CookHeader(
            title = recipe.title,
            status = status,
            steps = steps.size,
            index = index,
            finished = finished,
            onExit = onClose,
            onIngredients = { showIngredients = true },
            onGo = ::go,
        )

        val threshold = with(LocalDensity.current) { 60.dp.toPx() }
        Box(
            Modifier
                .weight(1f)
                .fillMaxWidth()
                .pointerInput(index, finished, steps.size) {
                    var dx = 0f
                    var dy = 0f
                    detectHorizontalDragGestures(
                        onDragStart = { dx = 0f; dy = 0f },
                        onDragEnd = {
                            when (swipeDirection(dx, dy, threshold)) {
                                1 -> next()
                                -1 -> prev()
                            }
                        },
                        onHorizontalDrag = { change, amount -> dx += amount; dy += change.positionChange().y },
                    )
                },
        ) {
            if (steps.isEmpty()) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text("This recipe has no steps yet.", style = CrumbText.body.copy(fontSize = 18.sp), color = c.inkMuted)
                }
            } else {
                val shift = with(LocalDensity.current) { 40.dp.roundToPx() * direction }
                AnimatedContent(
                    targetState = finished to index,
                    transitionSpec = {
                        slideInHorizontally(tween(280)) { shift } togetherWith slideOutHorizontally(tween(180)) { -shift }
                    },
                    label = "cook-step",
                ) { (done, i) ->
                    if (done) {
                        FinishedPage(onBack = onClose, onStartOver = { go(0) })
                    } else {
                        steps.getOrNull(i)?.let { stepAt ->
                            StepPage(
                                step = stepAt,
                                number = i + 1,
                                scale = scale,
                                ingredients = ingredients,
                                startTimer = startTimer,
                            )
                        }
                    }
                }
            }
        }

        CookFooter(
            index = index,
            finished = finished,
            hasSteps = steps.isNotEmpty(),
            isLast = steps.isNotEmpty() && index + 1 >= steps.size,
            onPrev = ::prev,
            onNext = ::next,
            onClose = onClose,
        )
    }

    if (showIngredients) {
        IngredientsSheet(
            ingredients = ingredients,
            scale = scale,
            onScale = { RecipeSession.setScale(recipe.id, it) },
            checked = checked.value,
            onToggle = { i -> checked.value = if (i in checked.value) checked.value - i else checked.value + i },
            onDismiss = { showIngredients = false },
        )
    }
}

/** The tile chrome: exit, title and status, the segment progress, and the ingredients button. */
@Composable
private fun CookHeader(
    title: String,
    status: String?,
    steps: Int,
    index: Int,
    finished: Boolean,
    onExit: () -> Unit,
    onIngredients: () -> Unit,
    onGo: (Int) -> Unit,
) {
    val c = Crumb.colors
    Column(Modifier.fillMaxWidth().tileSurface(c).statusBarsPadding()) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            TileIconButton(Lucide.X, "Exit cook mode", onExit)
            Column(
                Modifier.weight(1f).padding(horizontal = 8.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(
                    title,
                    style = TextStyle(fontFamily = DmSerif, fontSize = 22.sp, lineHeight = 26.sp),
                    color = c.onTile,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    textAlign = TextAlign.Center,
                )
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    if (status != null) {
                        Text(status, style = CrumbText.meta, color = c.onTile)
                        Text("·", style = CrumbText.meta, color = c.onTile)
                    }
                    Icon(Lucide.Sun, null, tint = c.onTile, modifier = Modifier.size(14.dp))
                    Text("Screen stays on", style = CrumbText.meta, color = c.onTile)
                }
            }
            TileIconButton(Lucide.List, "Ingredients", onIngredients)
        }

        Row(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            repeat(steps) { i ->
                val active = i == index && !finished
                Box(
                    Modifier.weight(1f).height(44.dp).clickable(role = Role.Button) { onGo(i) },
                    contentAlignment = Alignment.Center,
                ) {
                    Box(
                        Modifier
                            .fillMaxWidth()
                            .height(if (active) 10.dp else 6.dp)
                            .clip(RoundedCornerShape(50))
                            .background(if (i <= index || finished) c.onTile else c.onTile.copy(alpha = 0.3f)),
                    )
                }
            }
        }
        Spacer(Modifier.height(4.dp))
    }
}

/** A 48dp square icon button drawn on the tile: paper icon, a darken when pressed. */
@Composable
private fun TileIconButton(icon: ImageVector, contentDescription: String, onClick: () -> Unit) {
    val c = Crumb.colors
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    Box(
        Modifier
            .size(48.dp)
            .clip(ControlShape)
            .background(if (pressed) Color.Black.copy(alpha = 0.12f) else Color.Transparent)
            .clickable(interaction, indication = null, role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription, tint = c.onTile, modifier = Modifier.size(22.dp))
    }
}

/** One step: its number in serif, the text sized to fit, its timers and "You'll need". */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun StepPage(
    step: CookStep,
    number: Int,
    scale: Double,
    ingredients: List<IngredientLine>,
    startTimer: (String, Int) -> Unit,
) {
    val c = Crumb.colors
    val stepIngredients = remember(step, ingredients) {
        ingredientsForStep(step.text, step.section, ingredients).mapNotNull { ingredients.getOrNull(it.toInt()) }
    }
    val stepTimers = remember(step) { findTimers(step.text) }
    val size = stepTextSizeSp(step.text.length)

    Column(
        Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Column(Modifier.widthIn(max = 720.dp).fillMaxWidth()) {
            val section = step.section
            if (!section.isNullOrEmpty()) {
                Text(
                    section.uppercase(),
                    style = CrumbText.kicker,
                    color = c.primary,
                    modifier = Modifier.padding(bottom = 12.dp),
                )
            }
            Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.Top) {
                Text(
                    "$number",
                    style = TextStyle(
                        fontFamily = DmSerif,
                        fontSize = 64.sp,
                        lineHeight = 58.sp,
                        fontFeatureSettings = "tnum",
                    ),
                    color = c.primary,
                )
                Text(
                    step.text,
                    style = TextStyle(
                        fontFamily = NunitoSans,
                        fontSize = size.sp,
                        lineHeight = (size * 1.375).sp,
                        fontWeight = FontWeight.SemiBold,
                    ),
                    color = c.ink,
                    modifier = Modifier.weight(1f),
                )
            }

            if (stepTimers.isNotEmpty()) {
                FlowRow(
                    Modifier.padding(top = 28.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    stepTimers.forEach { timer ->
                        Btn(
                            "Start ${timer.label} timer",
                            { startTimer("Step $number: ${timer.label}", timer.seconds.toInt()) },
                            style = BtnStyle.Tile,
                            size = BtnSize.Lg,
                            icon = Lucide.AlarmClock,
                        )
                    }
                }
            }

            if (stepIngredients.isNotEmpty()) {
                Card(Modifier.padding(top = 28.dp), padding = PaddingValues(16.dp)) {
                    Text(
                        "You'll need".uppercase(),
                        style = CrumbText.meta.copy(letterSpacing = 0.08.em),
                        color = c.inkMuted,
                        modifier = Modifier.padding(bottom = 12.dp),
                    )
                    FlowRow(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        stepIngredients.forEach { ing ->
                            Box(
                                Modifier
                                    .clip(ControlShape)
                                    .background(c.tint)
                                    .padding(horizontal = 16.dp, vertical = 8.dp),
                            ) {
                                Text(
                                    scaleIngredient(ing.raw, scale),
                                    style = CrumbText.body.copy(
                                        fontSize = 17.sp,
                                        lineHeight = 23.sp,
                                        fontWeight = FontWeight.SemiBold,
                                    ),
                                    color = c.ink,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

/** "Bon appétit!" (spec §5.9): reaching the end logs a cook once, then this. */
@Composable
private fun FinishedPage(onBack: () -> Unit, onStartOver: () -> Unit) {
    val c = Crumb.colors
    Column(
        Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Box(
            Modifier.size(96.dp).clip(CardShape).tileSurface(c),
            contentAlignment = Alignment.Center,
        ) {
            Icon(Lucide.ChefHat, null, tint = c.onTile, modifier = Modifier.size(44.dp))
        }
        Text(
            "Bon appétit!",
            style = CrumbText.pageTitle.copy(fontSize = 36.sp, lineHeight = 38.sp),
            color = c.ink,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 20.dp),
        )
        Text(
            "That's every step. Enjoy it.",
            style = CrumbText.body.copy(fontSize = 18.sp),
            color = c.inkMuted,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 8.dp),
        )
        FlowRow(
            Modifier.padding(top = 32.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterHorizontally),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Btn("Back to recipe", onBack, style = BtnStyle.Primary, size = BtnSize.Xl)
            Btn("Start over", onStartOver, style = BtnStyle.Soft, size = BtnSize.Xl)
        }
    }
}

/** Big thumb-zone controls (spec §5.10): a 64dp previous and a full-width next/Finish/Close. */
@Composable
private fun CookFooter(
    index: Int,
    finished: Boolean,
    hasSteps: Boolean,
    isLast: Boolean,
    onPrev: () -> Unit,
    onNext: () -> Unit,
    onClose: () -> Unit,
) {
    val c = Crumb.colors
    Column(Modifier.fillMaxWidth().background(c.paper)) {
        HorizontalDivider(thickness = 1.dp, color = c.line)
        Row(
            Modifier
                .fillMaxWidth()
                .navigationBarsPadding()
                .padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Btn(
                text = null,
                onClick = onPrev,
                style = BtnStyle.Neutral,
                size = BtnSize.Xl,
                icon = Lucide.ArrowLeft,
                enabled = index > 0 || finished,
                contentDescription = "Previous step",
                modifier = Modifier.size(64.dp),
            )
            if (!finished) {
                Btn(
                    if (isLast) "Finish" else "Next step",
                    onNext,
                    style = BtnStyle.Primary,
                    size = BtnSize.Xl,
                    trailingIcon = if (isLast) Lucide.PartyPopper else Lucide.ArrowRight,
                    enabled = hasSteps,
                    modifier = Modifier.weight(1f),
                )
            } else {
                Btn("Close", onClose, style = BtnStyle.Outline, size = BtnSize.Xl, modifier = Modifier.weight(1f))
            }
        }
    }
}

/** The ingredients as a paper sheet: scale, section headings, checkable lines (spec §5.8). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun IngredientsSheet(
    ingredients: List<IngredientLine>,
    scale: Double,
    onScale: (Double) -> Unit,
    checked: Set<Int>,
    onToggle: (Int) -> Unit,
    onDismiss: () -> Unit,
) {
    val c = Crumb.colors
    val rows = remember(ingredients) { ingredientRows(ingredients) }
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheet, containerColor = c.paper) {
        LazyColumn(Modifier.fillMaxWidth().navigationBarsPadding().padding(bottom = 24.dp)) {
            item {
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = 20.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text("Ingredients", style = CrumbText.sectionTitle, color = c.ink, modifier = Modifier.weight(1f))
                    ScaleControl(scale, onScale)
                }
            }
            item { Spacer(Modifier.height(8.dp)) }
            items(rows.size) { i ->
                when (val row = rows[i]) {
                    is IngredientRow.Header -> Text(
                        row.text.uppercase(),
                        style = CrumbText.kicker,
                        color = c.primary,
                        modifier = Modifier.padding(start = 20.dp, end = 20.dp, top = 16.dp, bottom = 4.dp),
                    )
                    is IngredientRow.Line -> {
                        val isChecked = row.index in checked
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .clickable { onToggle(row.index) }
                                .padding(horizontal = 20.dp, vertical = 10.dp),
                            verticalAlignment = Alignment.Top,
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            Box(
                                Modifier
                                    .padding(top = 2.dp)
                                    .size(24.dp)
                                    .clip(RoundedCornerShape(50))
                                    .background(if (isChecked) c.tile else Color.Transparent)
                                    .then(
                                        if (isChecked) Modifier
                                        else Modifier.border(2.dp, c.lineStrong, RoundedCornerShape(50)),
                                    ),
                                contentAlignment = Alignment.Center,
                            ) {
                                if (isChecked) Icon(Lucide.Check, null, tint = c.onTile, modifier = Modifier.size(16.dp))
                            }
                            Text(
                                scaleIngredient(row.raw, scale),
                                style = CrumbText.body.copy(fontSize = 18.sp),
                                color = if (isChecked) c.inkMuted else c.ink,
                                textDecoration = if (isChecked) TextDecoration.LineThrough else null,
                                modifier = Modifier.weight(1f),
                            )
                        }
                    }
                }
            }
        }
    }
}

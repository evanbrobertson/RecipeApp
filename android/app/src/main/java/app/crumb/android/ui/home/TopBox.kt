package app.crumb.android.ui.home

import android.content.ClipData
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.content.MediaType
import androidx.compose.foundation.content.consume
import androidx.compose.foundation.content.contentReceiver
import androidx.compose.foundation.content.hasMediaType
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathOperation
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupPositionProvider
import androidx.compose.ui.window.PopupProperties
import app.crumb.android.data.Importer
import app.crumb.core.isVideoUrl
import app.crumb.core.jobProgress
import app.crumb.core.linksIn
import app.crumb.android.data.Incoming
import app.crumb.android.data.PhotoImport
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.CrumbNav
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.components.CardShape
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.theme.Caveat
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import coil3.compose.AsyncImage
import com.composables.icons.lucide.Check
import com.composables.icons.lucide.ChevronDown
import com.composables.icons.lucide.ChevronLeft
import com.composables.icons.lucide.ChevronRight
import com.composables.icons.lucide.ImagePlus
import com.composables.icons.lucide.LoaderCircle
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Plus
import com.composables.icons.lucide.Search
import com.composables.icons.lucide.X
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch

private const val MaxPhotos = PhotoImport.MAX_PHOTOS

/** The feet of the folder tabs: the inverted corners where the open tab meets the field. */
private val Foot = 12.dp

/**
 * The top box (web/src/islands/TopBox.svelte): Search and Add, switched by tabs like the
 * dividers in a recipe box. Add is one field; Crumb works out what was pasted and shows it
 * in a chip, which also opens a menu to pick the mode by hand. [tabs] off is /add's plain
 * Add block; [onTile] draws it for Home's tile header. [shared] prefills Add once.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun TopBox(
    modifier: Modifier = Modifier,
    tabs: Boolean = true,
    onTile: Boolean = true,
    recipeCount: Int? = null,
    shared: String? = null,
    onSharedUsed: () -> Unit = {},
    autofocus: Boolean = false,
) {
    val c = Crumb.colors
    val nav = LocalNav.current
    val container = AppContainerProvider
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    var tab by rememberSaveable { mutableStateOf(if (tabs) "search" else "add") }
    val query = rememberTextFieldState()
    val input = rememberTextFieldState()
    val pages = remember { mutableStateListOf<Uri>() }
    var file by remember { mutableStateOf<Uri?>(null) }
    var fileName by remember { mutableStateOf<String?>(null) }
    var manual by rememberSaveable { mutableStateOf<AddMode?>(null) }
    var menuOpen by remember { mutableStateOf(false) }
    var saving by remember { mutableStateOf(false) }
    var progress by remember { mutableStateOf("") }
    var vision by remember { mutableStateOf<Boolean?>(null) }
    var detected by remember { mutableStateOf(Detected(AddMode.Auto, "")) }
    var tipIndex by rememberSaveable { mutableStateOf(-1) }
    var fieldFocused by remember { mutableStateOf(false) }
    val addFocus = remember { FocusRequester() }
    val searchFocus = remember { FocusRequester() }

    fun pickTip() {
        var i = AddTips.indices.random()
        if (i == tipIndex) i = (i + 1) % AddTips.size
        tipIndex = i
    }

    // Shared into Crumb, or the /add route's text: straight into Add
    LaunchedEffect(shared) {
        if (!shared.isNullOrBlank()) {
            input.setTextAndPlaceCursorAtEnd(shared.trim())
            tab = "add"
            pickTip()
            onSharedUsed()
        }
    }
    LaunchedEffect(Unit) {
        if (autofocus) runCatching { if (tab == "add") addFocus.requestFocus() else searchFocus.requestFocus() }
    }
    // Debounced detection (≈250ms), so the chip doesn't flicker while typing
    LaunchedEffect(Unit) {
        snapshotFlow { Triple(input.text.toString(), pages.size, fileName) }.collectLatest { (text, photos, name) ->
            detected = when {
                photos > 0 -> Detected(AddMode.Photo, "$photos photo" + if (photos == 1) "" else "s")
                name != null -> Detected(AddMode.File, name)
                else -> {
                    if (text.isNotBlank()) delay(250)
                    detect(text)
                }
            }
            // Clearing the field hands the choice back to detection
            if (text.isBlank() && photos == 0 && name == null && manual != AddMode.Photo && manual != AddMode.File) manual = null
        }
    }
    LaunchedEffect(pages.isNotEmpty()) {
        if (pages.isNotEmpty() && vision == null) vision = container.importer.readsPhotos()
    }

    val text = input.text.toString()
    val mode = manual ?: detected.mode
    // What Add would do right now (the chip lags by the debounce; this doesn't)
    val now = manual ?: when {
        pages.isNotEmpty() -> AddMode.Photo
        file != null -> AddMode.File
        else -> detect(text).mode
    }
    val isVideo = linksIn(text).let { it.size == 1 && isVideoUrl(it.first()) }
    val summary = when {
        saving && mode == AddMode.Photo -> progress
        // A cooking video waits in the server's queue: say where it is, as the web does
        saving && mode == AddMode.Link && (progress.isNotEmpty() || isVideo) ->
            progress.ifEmpty { jobProgress("running", null) }
        saving -> if (mode == AddMode.Link) "Fetching… tricky sites take ~20s" else "Reading…"
        mode == AddMode.Photo -> if (pages.isNotEmpty()) detected.summary else "Up to $MaxPhotos pages of one recipe"
        mode == AddMode.Claude -> "Opens the connector setup"
        mode == AddMode.Apps -> "Opens the importer"
        mode == AddMode.File -> fileName ?: "Choose a file"
        manual != null && manual != detected.mode -> ""
        else -> detected.summary
    }
    val ready = !saving && canAdd(now, text, pages.size, photosReady = true)
    val photoNote = mode == AddMode.Photo && pages.isNotEmpty()
    // Photos read on this phone have no use for a note, so the field steps aside while empty
    val hideField = photoNote && vision == false && text.isBlank() && !fieldFocused

    /** Pasted or picked photos only take over an empty field (or one Photo was chosen for). */
    fun photosWelcome(): Boolean {
        if (saving) return false
        if (text.isBlank() || manual == AddMode.Photo) return true
        Toaster.show("Clear the text to add a photo", "Or pick Photo from the menu to keep it as a note.")
        return false
    }

    fun addPhotos(uris: List<Uri>) {
        if (saving || uris.isEmpty()) return
        val room = MaxPhotos - pages.size
        if (uris.size > room) Toaster.show("Up to $MaxPhotos photos", "They should all be one recipe.")
        val added = uris.take(room.coerceAtLeast(0))
        if (added.isEmpty()) return
        manual = AddMode.Photo
        file = null
        fileName = null
        pages.addAll(added)
    }

    /** Photos become pages; any other file is imported on its own. */
    fun takeFiles(uris: List<Uri>) {
        if (saving || uris.isEmpty()) return
        val images = uris.filter { context.contentResolver.getType(it)?.startsWith("image/") == true }
        if (images.isNotEmpty()) {
            if (photosWelcome()) addPhotos(images)
            return
        }
        pages.clear()
        file = uris.first()
        fileName = Incoming.displayName(context.contentResolver, uris.first()) ?: "File"
        manual = AddMode.File
    }

    val photoPicker = rememberLauncherForActivityResult(ActivityResultContracts.PickMultipleVisualMedia(MaxPhotos)) {
        if (it.isNotEmpty() && photosWelcome()) addPhotos(it)
    }
    val filePicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { takeFiles(it) }
    fun pickPhotos() = photoPicker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
    fun pickFile() = filePicker.launch(arrayOf("application/pdf", "text/*", "application/json", "image/*"))

    fun choose(m: AddMode) {
        manual = if (m == AddMode.Auto) null else m
        menuOpen = false
        if (m != AddMode.File) {
            file = null
            fileName = null
        }
        if (m != AddMode.Photo) pages.clear()
        when (m) {
            AddMode.File -> pickFile()
            AddMode.Photo -> if (pages.isEmpty()) pickPhotos()
            else -> runCatching { addFocus.requestFocus() }
        }
    }

    fun run(failTitle: String, block: suspend () -> Unit) {
        saving = true
        scope.launch {
            try {
                block()
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                Toaster.show(failTitle, e.friendlyMessage(), ToastTone.Error)
            } finally {
                saving = false
            }
        }
    }

    fun add() {
        if (!ready) return
        val trimmed = text.trim()
        when (now) {
            AddMode.Claude -> nav.connect()
            AddMode.Apps -> nav.import()
            AddMode.Scratch -> nav.newRecipe(trimmed)
            AddMode.Photo -> if (pages.isEmpty()) {
                pickPhotos()
            } else {
                progress = ""
                run("Couldn't read that photo") {
                    val note = if (vision == true) trimmed.takeIf { it.isNotEmpty() } else null
                    val saved = container.importer.photos(pages.toList(), note) { progress = it }
                    showOutcome(saved, nav)
                    pages.clear()
                    input.setTextAndPlaceCursorAtEnd("")
                }
            }
            AddMode.File -> {
                val picked = file
                if (picked == null) {
                    pickFile()
                } else if (Importer.unreadable(context.contentResolver.getType(picked), fileName)) {
                    Toaster.show("Crumb can't read that kind of file yet", "Open it, copy the recipe text, and paste it here instead.")
                } else {
                    run("Couldn't read that file") {
                        showOutcome(container.importer.files(listOf(picked)), nav)
                        file = null
                        fileName = null
                        manual = null
                    }
                }
            }
            AddMode.Link -> {
                val urls = linksIn(trimmed)
                if (urls.size > 1) {
                    nav.import(urls.joinToString("\n"))
                } else {
                    progress = ""
                    run("Couldn't get that recipe") {
                        showOutcome(container.importer.link(urls.first()) { progress = it }, nav)
                        input.setTextAndPlaceCursorAtEnd("")
                    }
                }
            }
            else -> run("Couldn't read that recipe") {
                showOutcome(container.importer.text(text), nav)
                input.setTextAndPlaceCursorAtEnd("")
            }
        }
    }

    // Switching tabs puts the cursor in the new field (opening Home doesn't)
    var switched by remember { mutableStateOf(0) }
    fun switchTab(next: String) {
        if (next == "add" && tab != "add") pickTip()
        tab = next
        menuOpen = false
        switched++
    }
    LaunchedEffect(switched) {
        if (switched == 0) return@LaunchedEffect
        delay(50)
        runCatching { if (tab == "add") addFocus.requestFocus() else searchFocus.requestFocus() }
    }

    val fieldBorder = if (onTile) Modifier else Modifier.border(1.dp, if (fieldFocused) c.tile else c.line, CardShape)
    Column(modifier.fillMaxWidth()) {
        if (tabs) {
            Box(Modifier.fillMaxWidth()) {
                Row(Modifier.padding(start = 16.dp + Foot), horizontalArrangement = Arrangement.spacedBy(Foot)) {
                    FolderTab("Search", Lucide.Search, tab == "search", onTile) { switchTab("search") }
                    FolderTab("Add", Lucide.Plus, tab == "add", onTile) { switchTab("add") }
                }
                if (onTile && tab == "add" && tipIndex >= 0) Tip(AddTips[tipIndex], Modifier.align(Alignment.BottomEnd))
            }
        }
        if (tab == "search") {
            Box(Modifier.fillMaxWidth().clip(CardShape).background(c.paper).then(fieldBorder)) {
                val placeholder = recipeCount?.let { "Search $it recipes" } ?: "Search recipes"
                BasicTextField(
                    state = query,
                    lineLimits = TextFieldLineLimits.SingleLine,
                    textStyle = TextStyle(color = c.ink, fontFamily = NunitoSans, fontSize = 16.sp),
                    cursorBrush = SolidColor(c.primary),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    onKeyboardAction = { nav.recipes(query.text.toString().trim().ifEmpty { null }) },
                    modifier = Modifier.fillMaxWidth().height(56.dp).focusRequester(searchFocus)
                        .onFocusChanged { fieldFocused = it.isFocused },
                    decorator = { field ->
                        Box(Modifier.fillMaxSize().padding(horizontal = 16.dp), contentAlignment = Alignment.CenterStart) {
                            if (query.text.isEmpty()) Text(placeholder, color = c.inkMuted, fontFamily = NunitoSans, fontSize = 16.sp)
                            field()
                        }
                    },
                )
            }
        } else {
            Box {
                Column(Modifier.fillMaxWidth().clip(CardShape).background(c.paper).then(fieldBorder)) {
                    if (photoNote) {
                        PhotoTray(
                            pages = pages,
                            saving = saving,
                            onAddPage = { pickPhotos() },
                            onMove = { i, by ->
                                val j = i + by
                                if (j in pages.indices) pages[i] = pages[j].also { pages[j] = pages[i] }
                            },
                            onRemove = { i -> pages.removeAt(i) },
                        )
                    }
                    if (!hideField) {
                        val placeholder = when {
                            !photoNote -> "Paste a link, a recipe, or type a name"
                            vision == false -> "Photos are read on this phone, so a note isn't used"
                            else -> "Add a note for Wee Chef, like which recipe on the page (optional)"
                        }
                        BasicTextField(
                            state = input,
                            enabled = !saving,
                            lineLimits = TextFieldLineLimits.MultiLine(maxHeightInLines = 12),
                            textStyle = TextStyle(color = c.ink, fontFamily = NunitoSans, fontSize = 16.sp, lineHeight = 23.sp),
                            cursorBrush = SolidColor(c.primary),
                            modifier = Modifier
                                .fillMaxWidth()
                                .heightIn(min = 56.dp)
                                .focusRequester(addFocus)
                                .onFocusChanged { fieldFocused = it.isFocused }
                                // A pasted screenshot or file becomes pages or an upload, as on the web
                                .contentReceiver { content ->
                                    // Rich text often carries a picture along; the text is the point
                                    if (content.hasMediaType(MediaType.Text)) return@contentReceiver content
                                    val uris = mutableListOf<Uri>()
                                    val rest = content.consume { item: ClipData.Item ->
                                        item.uri?.also { uris += it } != null
                                    }
                                    takeFiles(uris)
                                    rest
                                },
                            decorator = { field ->
                                Box(Modifier.padding(16.dp)) {
                                    if (input.text.isEmpty()) Text(placeholder, color = c.inkMuted, fontFamily = NunitoSans, fontSize = 16.sp)
                                    field()
                                }
                            },
                        )
                    }
                    HorizontalDivider(color = c.line)
                    Row(
                        Modifier.fillMaxWidth().padding(4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        ModeChip(mode.label, menuOpen) { menuOpen = !menuOpen }
                        Text(
                            summary,
                            color = c.inkMuted,
                            fontFamily = NunitoSans,
                            fontSize = 14.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                        AddButton(addLabel(mode, text), enabled = ready, saving = saving, onClick = ::add)
                    }
                }
                if (menuOpen) ModeMenu(mode, onChoose = ::choose, onDismiss = { menuOpen = false })
            }
        }
    }
}

/** One folder tab; the open one is paper and joins the field with inverted corners. */
@Composable
private fun FolderTab(label: String, icon: androidx.compose.ui.graphics.vector.ImageVector, selected: Boolean, onTile: Boolean, onClick: () -> Unit) {
    val c = Crumb.colors
    val shape = RoundedCornerShape(topStart = 12.dp, topEnd = 12.dp)
    val bg = when {
        selected -> c.paper
        onTile -> Color(0xFFFFFDF8).copy(alpha = 0.16f)
        else -> c.tint
    }
    val fg = when {
        selected -> c.ink
        onTile -> c.onTile
        else -> c.inkMuted
    }
    val paper = c.paper
    Row(
        Modifier
            // 1dp into the field, so no hairline of tile shows through the join
            .offset(y = if (selected) 1.dp else 0.dp)
            .drawBehind {
                if (!selected) return@drawBehind
                val f = Foot.toPx()
                val h = size.height
                fun foot(left: Float, circleX: Float) {
                    val square = Path().apply { addRect(Rect(left, h - f, left + f, h)) }
                    val bite = Path().apply { addOval(Rect(Offset(circleX, h - f), f)) }
                    drawPath(Path.combine(PathOperation.Difference, square, bite), paper)
                }
                foot(-f, -f)
                foot(size.width, size.width + f)
            }
            .height(44.dp)
            .clip(shape)
            .background(bg)
            .clickable(role = Role.Tab, onClick = onClick)
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(icon, contentDescription = null, tint = fg, modifier = Modifier.size(18.dp))
        Text(label, color = fg, fontFamily = NunitoSans, fontSize = 15.sp, fontWeight = if (selected) FontWeight.Bold else FontWeight.SemiBold)
    }
}

/** The handwritten tip beside the tabs, with a curl of an arrow down to the field. */
@Composable
private fun Tip(text: String, modifier: Modifier = Modifier) {
    val c = Crumb.colors
    val stroke = remember { PathParser().parsePathString("M2.5 13.5C10 8 20.5 9.5 21.5 18.5 22 24.5 19.5 29 18.5 33.5").toPath() }
    val head = remember { PathParser().parsePathString("M13 28.5Q16 31.5 18.3 34.5 21 31.2 24 28").toPath() }
    Row(modifier.offset(y = 6.dp), verticalAlignment = Alignment.Top) {
        Text(
            text,
            style = CrumbText.hand.copy(fontSize = 21.sp, lineHeight = 19.sp),
            color = c.onTile,
            modifier = Modifier.padding(top = 2.dp),
        )
        Box(
            Modifier.size(28.dp, 36.dp).drawBehind {
                val scale = size.width / 28f
                val style = Stroke(width = 1.8.dp.toPx(), cap = StrokeCap.Round)
                val m = androidx.compose.ui.graphics.Matrix().apply { scale(scale, scale) }
                listOf(stroke, head).forEach { p ->
                    val scaled = Path().apply { addPath(p) }.also { it.transform(m) }
                    drawPath(scaled, c.onTile, style = style)
                }
            },
        )
    }
}

/** The mode chip: "Auto-detect ▾" or the mode picked by hand. */
@Composable
private fun ModeChip(label: String, open: Boolean, onClick: () -> Unit) {
    val c = Crumb.colors
    val turn by animateFloatAsState(if (open) 180f else 0f, label = "chevron")
    Row(
        Modifier
            .height(44.dp)
            .clip(ControlShape)
            .background(c.tint)
            .clickable(role = Role.DropdownList, onClick = onClick)
            .padding(start = 14.dp, end = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(label, color = c.primary, fontFamily = NunitoSans, fontSize = 14.sp, fontWeight = FontWeight.Bold)
        Icon(Lucide.ChevronDown, contentDescription = null, tint = c.primary, modifier = Modifier.size(16.dp).rotate(turn))
    }
}

@Composable
private fun AddButton(label: String, enabled: Boolean, saving: Boolean, onClick: () -> Unit) {
    val c = Crumb.colors
    Row(
        Modifier
            .height(44.dp)
            .clip(ControlShape)
            .background(if (enabled) c.butter else c.tint)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(horizontal = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (saving) Icon(Lucide.LoaderCircle, contentDescription = null, tint = c.inkMuted, modifier = Modifier.size(16.dp))
        Text(label, color = if (enabled) c.onButter else c.inkMuted, fontFamily = NunitoSans, fontSize = 15.sp, fontWeight = FontWeight.Bold)
    }
}

/** The mode menu, dropped under the box and as wide as it (web `.menu`). */
@Composable
private fun ModeMenu(current: AddMode, onChoose: (AddMode) -> Unit, onDismiss: () -> Unit) {
    val c = Crumb.colors
    val gap = with(LocalDensity.current) { 8.dp.roundToPx() }
    val below = remember(gap) {
        object : PopupPositionProvider {
            override fun calculatePosition(anchorBounds: IntRect, windowSize: IntSize, layoutDirection: LayoutDirection, popupContentSize: IntSize) =
                IntOffset(anchorBounds.left, anchorBounds.bottom + gap)
        }
    }
    val width = with(LocalDensity.current) { androidx.compose.ui.platform.LocalWindowInfo.current.containerSize.width.toDp() } - 40.dp
    Popup(popupPositionProvider = below, onDismissRequest = onDismiss, properties = PopupProperties(focusable = true)) {
        Column(
            Modifier
                .width(width)
                .clip(CardShape)
                .background(c.paper)
                .border(1.dp, c.line, CardShape)
                .padding(vertical = 4.dp),
        ) {
            AddMode.entries.forEach { m ->
                Row(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = 56.dp)
                        .clickable(role = Role.RadioButton) { onChoose(m) }
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(m.label, color = c.ink, fontFamily = NunitoSans, fontSize = 15.sp, fontWeight = FontWeight.Bold)
                        Text(m.hint, color = c.inkMuted, fontFamily = NunitoSans, fontSize = 13.sp)
                    }
                    if (m == current) Icon(Lucide.Check, contentDescription = null, tint = c.primary, modifier = Modifier.size(20.dp))
                }
            }
        }
    }
}

/** The photos as pages, in order, with move, remove and "Add another page". */
@Composable
private fun PhotoTray(
    pages: List<Uri>,
    saving: Boolean,
    onAddPage: () -> Unit,
    onMove: (Int, Int) -> Unit,
    onRemove: (Int) -> Unit,
) {
    val c = Crumb.colors
    LazyRow(
        Modifier.fillMaxWidth().padding(top = 12.dp, bottom = 4.dp),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        itemsIndexed(pages, key = { _, uri -> uri.toString() }) { i, uri ->
            Column(Modifier.width(88.dp)) {
                Box(Modifier.fillMaxWidth().height(112.dp).clip(ControlShape).background(c.tint).border(1.dp, c.line, ControlShape)) {
                    AsyncImage(model = uri, contentDescription = "Page ${i + 1}", contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
                    Box(
                        Modifier.padding(6.dp).height(24.dp).clip(CircleShape).background(c.tile).padding(horizontal = 7.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text("${i + 1}", color = c.onTile, fontFamily = NunitoSans, fontSize = 13.sp, fontWeight = FontWeight.Bold)
                    }
                }
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    if (pages.size > 1) {
                        TrayButton(
                            if (i > 0) Lucide.ChevronLeft else Lucide.ChevronRight,
                            if (i > 0) "Move page ${i + 1} earlier" else "Move page 1 later",
                            enabled = !saving,
                        ) { onMove(i, if (i > 0) -1 else 1) }
                    } else {
                        Box(Modifier.size(44.dp))
                    }
                    TrayButton(Lucide.X, "Remove page ${i + 1}", enabled = !saving) { onRemove(i) }
                }
            }
        }
        if (pages.size < MaxPhotos) {
            item {
                Column(
                    Modifier
                        .width(88.dp)
                        .height(112.dp)
                        .clip(ControlShape)
                        .clickable(enabled = !saving, role = Role.Button, onClick = onAddPage)
                        .padding(horizontal = 8.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterVertically),
                ) {
                    Icon(Lucide.ImagePlus, contentDescription = null, tint = c.primary, modifier = Modifier.size(24.dp))
                    Text(
                        "Add another page",
                        color = c.primary,
                        fontFamily = NunitoSans,
                        fontSize = 13.sp,
                        fontWeight = FontWeight.Bold,
                        lineHeight = 16.sp,
                        textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                    )
                }
            }
        }
    }
}

@Composable
private fun TrayButton(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, enabled: Boolean, onClick: () -> Unit) {
    Box(
        Modifier.size(44.dp).clip(CircleShape).clickable(enabled = enabled, role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = label, tint = Crumb.colors.inkMuted, modifier = Modifier.size(20.dp))
    }
}

/** Where an Add or a share lands: the recipe, its editor to check, a cookbook, or the list. */
suspend fun showOutcome(outcome: app.crumb.android.data.ImportOutcome, nav: CrumbNav) {
    when (outcome) {
        is app.crumb.android.data.ImportOutcome.Saved -> if (outcome.onDevice) {
            Toaster.show("Check the amounts", "Photos are read on this device, so a few numbers or words may be off.")
            nav.edit(outcome.id)
        } else {
            when {
                !outcome.isNew -> Toaster.show("Already in your recipes")
                outcome.droppedPhoto -> Toaster.show(
                    "Saved without its photo",
                    "The site's photo link doesn't work. You can add one in Edit.",
                )
                outcome.fromVideo -> Toaster.show("Saved from the video", tone = ToastTone.Success)
            }
            nav.recipe(outcome.id)
        }
        is app.crumb.android.data.ImportOutcome.Book -> {
            val b = outcome.cookbook
            Toaster.show(
                app.crumb.core.bookImportedTitle(b.name, b.added.toUInt(), b.duplicates.toUInt(), b.skipped.toUInt()),
                tone = if (b.added > 0) ToastTone.Success else ToastTone.Default,
            )
            nav.cookbook(b.id)
        }
        is app.crumb.android.data.ImportOutcome.Files -> {
            val created = outcome.created
            outcome.results.firstOrNull { it.error != null }?.let { Toaster.show("Couldn't read that file", it.error, ToastTone.Error) }
            when {
                created.size == 1 -> nav.recipe(created.first().id)
                created.size > 1 -> {
                    Toaster.show("Imported ${created.size} recipes", tone = ToastTone.Success)
                    nav.recipes()
                }
                outcome.results.all { it.error == null } -> Toaster.show("Already in your recipes")
            }
        }
    }
}

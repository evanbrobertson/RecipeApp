package app.crumb.android.ui.recipe

import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.graphics.Color as AndroidColor
import android.view.View
import android.view.ViewGroup
import android.webkit.RenderProcessGoneDetail
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.systemGestureExclusion
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.core.net.toUri
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.Lifecycle.Event
import androidx.lifecycle.LifecycleEventObserver
import androidx.media3.common.MediaItem
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.PlayerView
import app.crumb.android.data.VideoEmbed
import app.crumb.android.data.VideoWin
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.components.Card
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import coil3.compose.AsyncImage
import com.composables.icons.lucide.ArrowUp
import com.composables.icons.lucide.ExternalLink
import com.composables.icons.lucide.GripHorizontal
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.MoveDiagonal
import com.composables.icons.lucide.MoveDiagonal2
import com.composables.icons.lucide.Play
import com.composables.icons.lucide.X
import kotlinx.coroutines.launch
import kotlin.math.roundToInt

// The recipe's video (web/src/components/RecipeVideo.svelte). The card sits in the page after the
// ingredients; nothing loads from the video's site until Play. The player itself is not inside
// the list: a LazyColumn drops items that scroll away, which would stop it, so one player lives
// over the page and follows the card's slot (or floats in a window once the card scrolls past).
// Moving it between the two only changes its modifiers, so it never reloads or restarts.
// This is the app's one WebView: YouTube, Vimeo, TikTok and the rest only offer web players.

/** What the card and the player share: whether it plays, where the slot is, the window and fullscreen. */
@Stable
class VideoState {
    var playing by mutableStateOf(false)

    /** The slot's left edge and size in px, and its top below the top of the list item holding it. */
    var slotX by mutableFloatStateOf(0f)
    var slotWidth by mutableIntStateOf(0)
    var slotHeight by mutableIntStateOf(0)
    var slotTopInItem by mutableFloatStateOf(0f)

    // The item's outer edge and the slot, as laid out; the slot's top below the item's is read from
    // both together, so neither callback's order (nor the card's own top padding) can skew it
    private var itemCoords: LayoutCoordinates? = null
    private var slotCoords: LayoutCoordinates? = null

    fun itemPlaced(coords: LayoutCoordinates) {
        itemCoords = coords
        measureSlot()
    }

    fun slotPlaced(coords: LayoutCoordinates) {
        slotCoords = coords
        val at = coords.positionInRoot()
        slotX = at.x
        slotWidth = coords.size.width
        slotHeight = coords.size.height
        measureSlot()
    }

    private fun measureSlot() {
        val item = itemCoords?.takeIf { it.isAttached } ?: return
        val slot = slotCoords?.takeIf { it.isAttached } ?: return
        slotTopInItem = item.localPositionOf(slot, Offset.Zero).y
    }

    /** The floating window, once it has a place. */
    var win by mutableStateOf<VideoWin?>(null)

    /** A WebPlayer's fullscreen view (its own button), shown over everything. */
    var fullscreen by mutableStateOf<View?>(null)
    var onFullscreenHidden: WebChromeClient.CustomViewCallback? = null

    fun stop() {
        playing = false
    }

    companion object {
        /** Keeps "playing" through a recreated activity (a rotation, a font size change); the player itself starts over. */
        val Saver = androidx.compose.runtime.saveable.listSaver<VideoState, Boolean>(
            save = { listOf(it.playing) },
            restore = { VideoState().apply { playing = it[0] } },
        )
    }
}

/**
 * The card in the page: a "Video" title with "Watch on …", then the still with its play button, or
 * (for a site that can't play in place) just a link. Once playing, the still's place stays empty
 * for the player laid over it.
 */
@Composable
fun RecipeVideoCard(video: String, embed: VideoEmbed?, title: String, state: VideoState, modifier: Modifier = Modifier) {
    val c = Crumb.colors
    val context = LocalContext.current
    val host = app.crumb.core.hostOf(video)

    // Measured outside the caller's padding, so it is the list item's own top
    Column(Modifier.onGloballyPositioned { state.itemPlaced(it) }.then(modifier)) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = 44.dp).padding(bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("Video", style = CrumbText.sectionTitle, color = c.ink, modifier = Modifier.weight(1f))
            Row(
                Modifier
                    .heightIn(min = 44.dp)
                    .clickable(role = Role.Button) { openLink(context, embed?.watchUrl ?: video) },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(
                    watchLabel(embed, host),
                    color = c.primary,
                    fontFamily = NunitoSans,
                    fontSize = 14.sp,
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Icon(Lucide.ExternalLink, null, tint = c.primary, modifier = Modifier.size(14.dp))
            }
        }
        if (embed == null) {
            Card(Modifier.fillMaxWidth().clickable(role = Role.Button) { openLink(context, video) }) {
                Row(
                    Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    PlayTile(36.dp, 16.dp)
                    Text("Watch the video on ${host ?: "its site"}", style = CrumbText.rowTitle, color = c.ink)
                }
            }
        } else {
            var noStill by remember(embed.thumbnail) { mutableStateOf(false) }
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                Box(
                    Modifier
                        .then(if (embed.vertical) Modifier.widthIn(max = 352.dp).fillMaxWidth().aspectRatio(9f / 16f) else Modifier.fillMaxWidth().aspectRatio(16f / 9f))
                        .clip(ControlShape)
                        .background(c.tint)
                        .onGloballyPositioned { state.slotPlaced(it) },
                ) {
                    if (!state.playing) {
                        Box(
                            Modifier
                                .fillMaxSize()
                                .clickable(role = Role.Button, onClickLabel = "Play the video: $title") { state.playing = true }
                                .semantics { contentDescription = "Play the video: $title" },
                            contentAlignment = Alignment.Center,
                        ) {
                            if (embed.thumbnail != null && !noStill) {
                                AsyncImage(
                                    model = embed.thumbnail,
                                    contentDescription = null,
                                    contentScale = ContentScale.Crop,
                                    onError = { noStill = true },
                                    modifier = Modifier.fillMaxSize(),
                                )
                            }
                            PlayTile(64.dp, 28.dp)
                        }
                    }
                }
            }
        }
    }
}

/** The tile-green circle with a filled play triangle (nudged right to look centred). */
@Composable
private fun PlayTile(size: androidx.compose.ui.unit.Dp, icon: androidx.compose.ui.unit.Dp) {
    val c = Crumb.colors
    Box(Modifier.size(size).clip(CircleShape).background(c.tile), contentAlignment = Alignment.Center) {
        Icon(Lucide.Play, null, tint = c.onTile, modifier = Modifier.padding(start = icon / 14).size(icon))
    }
}

/**
 * The player over the page: in the card's slot while that is on screen, and in a floating window
 * (bar on top: move, back to the card, stop, resize) once the card is scrolled past. The card is
 * item [videoIndex] (keyed [videoKey]) of [list]; [origin] is where the list's top left is in the window.
 */
@Composable
fun RecipeVideoPlayer(
    state: VideoState,
    embed: VideoEmbed,
    title: String,
    list: LazyListState,
    videoIndex: Int,
    videoKey: Any,
    origin: Offset,
) {
    if (!state.playing) return
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    val local = AppContainerProvider.local
    val tall = embed.vertical

    // The window's screen, in dp, clear of the system bars
    val size = LocalWindowInfo.current.containerSize
    val topBar = WindowInsets.statusBars.getTop(density)
    val bottomBar = WindowInsets.navigationBars.getBottom(density)
    val screen = with(density) {
        VideoScreen(size.width.toDp().value, size.height.toDp().value, topBar.toDp().value, bottomBar.toDp().value)
    }

    // The slot's top in the list, read from its layout so the player tracks the scroll exactly;
    // out of the list it is a screen above or below it
    fun slotTop(): Float {
        val item = list.layoutInfo.visibleItemsInfo.firstOrNull { it.key == videoKey }
        return when {
            item != null -> item.offset + state.slotTopInItem
            list.firstVisibleItemIndex > videoIndex -> -state.slotHeight.toFloat() - 1f
            else -> list.layoutInfo.viewportSize.height.toFloat()
        }
    }

    val away by remember {
        derivedStateOf { slotAway(slotTop(), state.slotHeight.toFloat(), list.layoutInfo.viewportSize.height.toFloat(), atEnd = !list.canScrollForward) }
    }
    val floating = away && state.win != null

    // First float: where it was left, else the default place
    LaunchedEffect(away, screen) {
        if (!away) return@LaunchedEffect
        val current = state.win ?: local.videoWindow(tall) ?: VideoWindow.firstPlace(tall, screen)
        state.win = VideoWindow.fit(current, tall, screen)
    }

    fun backToVideo() {
        scope.launch {
            val viewport = list.layoutInfo.viewportSize.height
            val offset = -(viewport / 2f - state.slotTopInItem - state.slotHeight / 2f)
            list.animateScrollToItem(videoIndex, offset.roundToInt().coerceAtMost(0))
        }
    }

    val win = state.win
    val frame = if (floating && win != null) {
        Modifier
            .offset { IntOffset((win.x * density.density - origin.x).roundToInt(), (win.y * density.density - origin.y).roundToInt()) }
            .size(win.w.dp, VideoWindow.height(win.w, tall).dp)
            .shadow(8.dp, ControlShape)
    } else {
        Modifier
            .offset { IntOffset((state.slotX - origin.x).roundToInt(), slotTop().roundToInt()) }
            .size(with(density) { state.slotWidth.toDp() }, with(density) { state.slotHeight.toDp() })
    }

    Column(
        frame
            .clip(ControlShape)
            .background(Color.Black),
    ) {
        if (floating && win != null) {
            FloatingBar(
                win = win,
                tall = tall,
                screen = screen,
                onWin = { state.win = it },
                onSettled = { local.setVideoWindow(tall, it) },
                onBack = ::backToVideo,
                onStop = state::stop,
            )
        }
        if (embed.provider == "file") {
            FilePlayer(embed.embedUrl, Modifier.weight(1f).fillMaxWidth())
        } else {
            WebPlayer(embed, state, Modifier.weight(1f).fillMaxWidth(), onGone = state::stop)
        }
    }

    state.fullscreen?.let { view ->
        val hide = {
            state.onFullscreenHidden?.onCustomViewHidden()
            state.fullscreen = null
        }
        Dialog(
            onDismissRequest = hide,
            properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false),
        ) {
            AndroidView(factory = { view }, modifier = Modifier.fillMaxSize().background(Color.Black))
        }
    }
}

/**
 * The floating window's bar: resize handle, grip (drag to move), "Back to video", stop, resize
 * handle. Each button is 40dp; "Back to video" drops out when the window is too narrow for them.
 */
@Composable
private fun FloatingBar(
    win: VideoWin,
    tall: Boolean,
    screen: VideoScreen,
    onWin: (VideoWin) -> Unit,
    onSettled: (VideoWin) -> Unit,
    onBack: () -> Unit,
    onStop: () -> Unit,
) {
    val c = Crumb.colors
    Row(
        // The resize handles sit at the window's corners: keep them (and the bar) clear of the
        // system's back gesture at the screen's edges
        Modifier.fillMaxWidth().height(VideoWindow.BAR.dp).background(c.tile).systemGestureExclusion(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        DragHandle(Lucide.MoveDiagonal2, "Resize the video", VideoDrag.Left, win, tall, screen, onWin, onSettled)
        DragHandle(Lucide.GripHorizontal, "Move the video", VideoDrag.Move, win, tall, screen, onWin, onSettled, Modifier.weight(1f), dim = true)
        if (VideoWindow.showBack(win.w)) {
            Row(
                Modifier
                    .height(VideoWindow.BAR.dp)
                    .clickable(role = Role.Button, onClickLabel = "Back to video", onClick = onBack)
                    .padding(horizontal = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Icon(Lucide.ArrowUp, "Back to video", tint = c.onTile, modifier = Modifier.size(16.dp))
                if (VideoWindow.showBackLabel(win.w)) {
                    Text("Back to video", color = c.onTile, fontFamily = NunitoSans, fontSize = 13.sp, fontWeight = FontWeight.Bold, maxLines = 1)
                }
            }
        }
        Box(
            Modifier.size(VideoWindow.BAR.dp).clickable(role = Role.Button, onClickLabel = "Stop the video", onClick = onStop),
            contentAlignment = Alignment.Center,
        ) {
            Icon(Lucide.X, "Stop the video", tint = c.onTile, modifier = Modifier.size(16.dp))
        }
        DragHandle(Lucide.MoveDiagonal, "Resize the video", VideoDrag.Right, win, tall, screen, onWin, onSettled)
    }
}

/** A bar button that moves or resizes the window as it's dragged, and remembers the result. */
@Composable
private fun DragHandle(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    label: String,
    kind: VideoDrag,
    win: VideoWin,
    tall: Boolean,
    screen: VideoScreen,
    onWin: (VideoWin) -> Unit,
    onSettled: (VideoWin) -> Unit,
    modifier: Modifier = Modifier,
    dim: Boolean = false,
) {
    val c = Crumb.colors
    val density = LocalDensity.current
    val latest by rememberUpdatedState(win)
    val screenNow by rememberUpdatedState(screen)
    var from by remember { mutableStateOf(win) }
    var total by remember { mutableStateOf(Offset.Zero) }
    Box(
        modifier
            .height(VideoWindow.BAR.dp)
            .then(if (kind == VideoDrag.Move) Modifier else Modifier.width(VideoWindow.BAR.dp))
            .pointerInput(kind, tall) {
                detectDragGestures(
                    onDragStart = {
                        from = latest
                        total = Offset.Zero
                    },
                    onDragEnd = { onSettled(latest) },
                    onDragCancel = { onSettled(latest) },
                ) { change, amount ->
                    change.consume()
                    total += amount
                    onWin(VideoWindow.apply(kind, from, total.x / density.density, total.y / density.density, tall, screenNow))
                }
            }
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, null, tint = c.onTile.copy(alpha = if (dim) 0.7f else 1f), modifier = Modifier.size(if (dim) 20.dp else 16.dp))
    }
}

/** A site's web player: an iframe in a WebView, kept alive while it moves between the page and the window. */
@SuppressLint("SetJavaScriptEnabled")
@Composable
private fun WebPlayer(embed: VideoEmbed, state: VideoState, modifier: Modifier, onGone: () -> Unit) {
    val context = LocalContext.current
    val owner = LocalLifecycleOwner.current
    val html = remember(embed) { playerHtml(embed) }
    if (html == null) {
        LaunchedEffect(embed) { onGone() }
        return
    }
    val goneNow by rememberUpdatedState(onGone)
    var view by remember { mutableStateOf<WebView?>(null) }
    // Paused with the app, so it doesn't play on with the screen off
    DisposableEffect(owner, view) {
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Event.ON_PAUSE -> view?.onPause()
                Event.ON_RESUME -> view?.onResume()
                else -> Unit
            }
        }
        owner.lifecycle.addObserver(observer)
        onDispose { owner.lifecycle.removeObserver(observer) }
    }
    AndroidView(
        modifier = modifier,
        factory = {
            WebView(context).apply {
                setBackgroundColor(AndroidColor.BLACK)
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = true
                settings.mediaPlaybackRequiresUserGesture = false
                settings.allowFileAccess = false
                settings.allowContentAccess = false
                webViewClient = object : WebViewClient() {
                    // The embed stays in its frame: a page it tries to open goes to the browser
                    override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                        if (request.isForMainFrame) openLink(context, request.url.toString())
                        return request.isForMainFrame
                    }

                    override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
                        goneNow()
                        return true
                    }
                }
                webChromeClient = object : WebChromeClient() {
                    override fun onShowCustomView(v: View, callback: CustomViewCallback) {
                        state.fullscreen = v
                        state.onFullscreenHidden = callback
                    }

                    override fun onHideCustomView() {
                        state.fullscreen = null
                        state.onFullscreenHidden = null
                    }
                }
                // YouTube refuses an embed with no identifiable https origin (error 153)
                loadDataWithBaseURL("https://${context.packageName}/", html, "text/html", "utf-8", null)
                view = this
            }
        },
        onRelease = {
            view = null
            state.fullscreen = null
            (it.parent as? ViewGroup)?.removeView(it)
            it.stopLoading()
            it.destroy()
        },
    )
}

/** A recipe's own video file, in ExoPlayer; released when it stops or the page closes. */
@Composable
private fun FilePlayer(url: String, modifier: Modifier) {
    val context = LocalContext.current
    val player = remember(url) {
        ExoPlayer.Builder(context).build().apply {
            setMediaItem(MediaItem.fromUri(url))
            playWhenReady = true
            prepare()
        }
    }
    DisposableEffect(player) { onDispose { player.release() } }
    AndroidView(
        modifier = modifier,
        factory = { PlayerView(it).apply { this.player = player; setBackgroundColor(AndroidColor.BLACK) } },
        onRelease = { it.player = null },
    )
}

/** Hands a link to whatever app opens it (the YouTube or TikTok app, else the browser). */
private fun openLink(context: Context, url: String) {
    runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, url.toUri()).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
}

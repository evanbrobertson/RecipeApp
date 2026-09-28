package app.crumb.android.ui.more

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.content.pm.PackageManager
import android.location.Location
import android.location.LocationListener
import android.location.LocationManager
import android.os.Build
import android.os.Looper
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import app.crumb.android.BuildConfig
import app.crumb.android.data.ApiException
import app.crumb.android.data.ConnectorInfo
import app.crumb.android.data.CrumbApi
import app.crumb.android.data.Download
import app.crumb.android.data.ShareKind
import app.crumb.android.data.SharedLink
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.Card
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.GroupLabel
import app.crumb.android.ui.components.ListCard
import app.crumb.android.ui.components.ListRow
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.SunClock
import app.crumb.android.ui.theme.SunLocation
import app.crumb.android.ui.theme.ThemeMode
import app.crumb.android.ui.theme.ThemeSettings
import com.composables.icons.lucide.Archive
import com.composables.icons.lucide.BookOpen
import com.composables.icons.lucide.ChefHat
import com.composables.icons.lucide.ChevronRight
import com.composables.icons.lucide.Check
import com.composables.icons.lucide.ClipboardCheck
import com.composables.icons.lucide.CookingPot
import com.composables.icons.lucide.Copy
import com.composables.icons.lucide.Download
import com.composables.icons.lucide.ExternalLink
import com.composables.icons.lucide.FileDown
import com.composables.icons.lucide.Globe
import com.composables.icons.lucide.LoaderCircle
import com.composables.icons.lucide.LocateFixed
import com.composables.icons.lucide.LogOut
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.MonitorSmartphone
import com.composables.icons.lucide.Moon
import com.composables.icons.lucide.PenLine
import com.composables.icons.lucide.Plug
import com.composables.icons.lucide.Server
import com.composables.icons.lucide.Shuffle
import com.composables.icons.lucide.Sun
import com.composables.icons.lucide.Sunrise
import com.composables.icons.lucide.Unlink
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.coroutines.resume
import kotlin.math.round

/** Everything the More page reads from the server (web MoreSettings' page data). */
data class MoreUiState(
    val connector: ConnectorInfo? = null,
    /** null until the first load lands; empty means no live links. */
    val shares: List<SharedLink>? = null,
    val error: String? = null,
    val signedOut: Boolean = false,
    val stopping: Boolean = false,
)

class MoreViewModel(private val api: CrumbApi) : ViewModel() {
    private val _state = MutableStateFlow(MoreUiState())
    val state: StateFlow<MoreUiState> = _state.asStateFlow()

    init {
        load()
    }

    fun load() {
        _state.update { it.copy(error = null) }
        viewModelScope.launch {
            try {
                val connector = api.connector()
                // The shares list is best-effort: a box that has never shared shows nothing
                val shares = runCatching { api.shares() }.getOrDefault(emptyList())
                _state.update { it.copy(connector = connector, shares = shares) }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _state.update {
                    it.copy(error = e.friendlyMessage(), signedOut = (e as? ApiException)?.isSignedOut == true)
                }
            }
        }
    }

    /** The web's `stopSharing`: delete by id, drop the row, tell the cook. */
    fun stopSharing(link: SharedLink) {
        if (_state.value.stopping) return
        _state.update { it.copy(stopping = true) }
        viewModelScope.launch {
            try {
                val kind = if (link.kind == "cookbook") ShareKind.Cookbook else ShareKind.Recipe
                api.stopSharing(kind, link.id)
                _state.update { it.copy(shares = it.shares?.filter { s -> s.url != link.url }) }
                Toaster.show("Stopped sharing", "The link no longer works.")
            } catch (e: Exception) {
                Toaster.show("Couldn't stop sharing", e.friendlyMessage(), ToastTone.Error)
            } finally {
                _state.update { it.copy(stopping = false) }
            }
        }
    }
}

/**
 * The More page (web/src/pages/more.astro + islands/MoreSettings.svelte): the shortcut rows,
 * the download, the theme picker with its sunrise/sunset panel, shared links and server info.
 */
@Composable
fun MoreScreen() {
    val container = AppContainerProvider
    val nav = LocalNav.current
    val context = LocalContext.current
    val uriHandler = LocalUriHandler.current
    val scope = rememberCoroutineScope()
    val vm: MoreViewModel = crumbViewModel { MoreViewModel(container.api) }
    val state by vm.state.collectAsStateWithLifecycle()
    val settings by container.theme.settings.collectAsStateWithLifecycle(initialValue = ThemeSettings())
    val clipboard = LocalClipboardManager.current
    val c = Crumb.colors

    var pending by remember { mutableStateOf<Download?>(null) }
    val backupSaver = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")) { uri ->
        val download = pending
        pending = null
        if (uri != null && download != null) {
            runCatching { context.contentResolver.openOutputStream(uri)?.use { it.write(download.bytes) } }
                .onSuccess { Toaster.show("Backup saved", tone = ToastTone.Success) }
                .onFailure { Toaster.show("Couldn't save that", it.message ?: "Try again.", ToastTone.Error) }
        }
    }

    var locating by remember { mutableStateOf(false) }
    fun findLocation() {
        if (locating) return
        locating = true
        scope.launch {
            val fix = withContext(Dispatchers.IO) { LocationReader.current(context) }
            locating = false
            if (fix == null) {
                Toaster.show("Couldn't find your location", "Still using the estimate from your time zone.", ToastTone.Error)
            } else {
                container.theme.setLocation(SunLocation(roundOne(fix.latitude), roundOne(fix.longitude)))
                Toaster.show("Using your location", "Sunrise and sunset are exact now.")
            }
        }
    }
    val locationPermission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) findLocation()
        else Toaster.show(
            "Location is blocked",
            "Allow location for Crumb in your phone's settings, or keep the time-zone estimate.",
            ToastTone.Error,
        )
    }
    fun locate() {
        if (locating) return
        val granted = context.checkSelfPermission(Manifest.permission.ACCESS_COARSE_LOCATION) == PackageManager.PERMISSION_GRANTED
        if (granted) findLocation() else locationPermission.launch(Manifest.permission.ACCESS_COARSE_LOCATION)
    }

    LaunchedEffect(state.signedOut) {
        if (state.signedOut) nav.signedOut()
    }

    val server = container.session.current?.server?.toString()?.trimEnd('/')
    val serverHost = server?.removePrefix("https://")?.removePrefix("http://")
    val saved = settings.location != null
    val effectiveLocation = remember(saved, settings.location) { settings.location ?: SunClock.estimate() }

    Column(
        Modifier.fillMaxSize().statusBarsPadding().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(28.dp),
    ) {
        Text("More", style = CrumbText.pageTitle, color = c.ink)
        state.error?.let { Text(it, style = CrumbText.bodySmall, color = c.error) }

        Group(
            "Your recipes",
            listOf(
                { ListRow(title = "Surprise me", icon = Lucide.Shuffle, plainIcon = true, onClick = { nav.surprise() }) },
                { ListRow(title = "Write a recipe", icon = Lucide.PenLine, plainIcon = true, onClick = { nav.newRecipe() }) },
                {
                    ListRow(
                        title = "Import recipes",
                        subtitle = "Links, files, Paprika, Mealie",
                        icon = Lucide.FileDown,
                        plainIcon = true,
                        onClick = { nav.import() },
                    )
                },
            ),
        )

        Group(
            "Claude & backups",
            listOf(
                { ListRow(title = "Connect to Claude", icon = Lucide.Plug, plainIcon = true, onClick = { nav.connect() }) },
                {
                    ListRow(
                        title = "Download a backup",
                        subtitle = "Everything as one JSON file",
                        icon = Lucide.Archive,
                        plainIcon = true,
                        onClick = {
                            scope.launch {
                                try {
                                    val download = container.api.exportAll()
                                    pending = download
                                    backupSaver.launch(download.fileName)
                                } catch (e: Exception) {
                                    Toaster.show("Couldn't download that", e.friendlyMessage(), ToastTone.Error)
                                }
                            }
                        },
                        trailing = { Icon(Lucide.Download, null, tint = c.inkMuted, modifier = Modifier.size(20.dp)) },
                    )
                },
            ),
        )

        ThemeSection(
            mode = settings.mode,
            saved = saved,
            location = effectiveLocation,
            locating = locating,
            onMode = { scope.launch { container.theme.setMode(it) } },
            onLocate = ::locate,
            onForget = { scope.launch { container.theme.setLocation(null) } },
        )

        val shares = state.shares
        if (!shares.isNullOrEmpty()) {
            Group(
                "Shared links",
                shares.map { link ->
                    {
                        ShareRow(
                            link = link,
                            stopping = state.stopping,
                            onCopy = {
                                runCatching {
                                    clipboard.setText(AnnotatedString(link.url))
                                    Toaster.show("Link copied")
                                }.onFailure { Toaster.show("Couldn't copy", tone = ToastTone.Error) }
                            },
                            onStop = { vm.stopSharing(link) },
                        )
                    }
                },
            )
        }

        val info = state.connector
        Group(
            "Settings",
            buildList<@Composable () -> Unit> {
                if (info != null) {
                    add {
                        ListRow(
                            title = "Tricky sites",
                            subtitle = if (info.browserScraping) "A real browser steps in when a site blocks us" else "Browser fallback not installed",
                            icon = Lucide.Globe,
                            plainIcon = true,
                            onClick = null,
                        )
                    }
                    add {
                        ListRow(
                            title = "Pasted text",
                            subtitle = if (info.weeChef) "Tidied up by Wee Chef" else "Read by the built-in parser",
                            icon = Lucide.ChefHat,
                            plainIcon = true,
                            onClick = null,
                            trailing = if (info.weeChef) {
                                { Text("Wee Chef is on", style = CrumbText.meta, color = c.inkMuted, maxLines = 1) }
                            } else null,
                        )
                    }
                    if (info.weeChefChecks) {
                        add {
                            ListRow(
                                title = "Wee Chef checks",
                                icon = Lucide.ClipboardCheck,
                                plainIcon = true,
                                onClick = { nav.suggestions() },
                                trailing = {
                                    Text("Suggestions", style = CrumbText.meta, color = c.inkMuted, modifier = Modifier.padding(end = 4.dp))
                                    Icon(Lucide.ChevronRight, null, tint = c.inkMuted, modifier = Modifier.size(20.dp))
                                },
                            )
                        }
                    }
                }
                add {
                    ListRow(
                        title = "Server",
                        subtitle = serverHost,
                        icon = Lucide.Server,
                        plainIcon = true,
                        onClick = server?.let { { uriHandler.openUri(it) } },
                        trailing = { Icon(Lucide.ExternalLink, null, tint = c.inkMuted, modifier = Modifier.size(20.dp)) },
                    )
                }
                if (info?.authEnabled != false) {
                    add {
                        ListRow(
                            title = "Sign out",
                            icon = Lucide.LogOut,
                            plainIcon = true,
                            onClick = { scope.launch { container.signOut() } },
                            chevron = false,
                        )
                    }
                }
            },
        )

        Text(
            "Crumb for Android ${BuildConfig.VERSION_NAME}",
            style = CrumbText.meta,
            color = c.inkMuted,
        )
    }
}

/** An uppercase group heading with its row card, as on the web's settings pages. */
@Composable
private fun Group(title: String, rows: List<@Composable () -> Unit>) {
    Column {
        GroupLabel(title)
        ListCard(rows = rows)
    }
}

@Composable
private fun ThemeSection(
    mode: ThemeMode,
    saved: Boolean,
    location: SunLocation,
    locating: Boolean,
    onMode: (ThemeMode) -> Unit,
    onLocate: () -> Unit,
    onForget: () -> Unit,
) {
    val c = Crumb.colors
    Column {
        GroupLabel("Theme")
        Card {
            ThemeMode.entries.forEachIndexed { i, entry ->
                if (i > 0) HorizontalDivider(Modifier.padding(horizontal = 12.dp), thickness = 1.dp, color = c.line)
                ListRow(
                    title = entry.label,
                    subtitle = entry.detail,
                    icon = themeIcon(entry),
                    plainIcon = true,
                    chevron = false,
                    onClick = { onMode(entry) },
                    trailing = {
                        Icon(
                            Lucide.Check,
                            null,
                            tint = if (entry == mode) c.primary else Color.Transparent,
                            modifier = Modifier.size(20.dp),
                        )
                    },
                )
            }
            if (mode == ThemeMode.Sun) {
                HorizontalDivider(Modifier.padding(horizontal = 12.dp), thickness = 1.dp, color = c.line)
                SunPanel(location = location, saved = saved, locating = locating, onLocate = onLocate, onForget = onForget)
            }
        }
    }
}

/** The "Sunrise & sunset" sub-panel: today's times, where they came from, and the buttons. */
@Composable
private fun SunPanel(location: SunLocation, saved: Boolean, locating: Boolean, onLocate: () -> Unit, onForget: () -> Unit) {
    val c = Crumb.colors
    Column(Modifier.padding(start = 50.dp, end = 16.dp, top = 12.dp, bottom = 12.dp)) {
        Text(
            sunLine(System.currentTimeMillis(), location),
            style = CrumbText.body.copy(fontWeight = FontWeight.Bold),
            color = c.ink,
            modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
        )
        Text(
            if (saved) "Using your saved location" else "Estimated from your time zone",
            style = CrumbText.meta,
            color = c.inkMuted,
            modifier = Modifier.padding(top = 2.dp),
        )
        Row(Modifier.padding(top = 10.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            LocateButton(saved = saved, locating = locating, onClick = onLocate)
            if (saved) Btn("Forget location", onForget, style = BtnStyle.Ghost)
        }
    }
}

/** "Use my location" / "Update my location", its loader turning while the fix is found. */
@Composable
private fun LocateButton(saved: Boolean, locating: Boolean, onClick: () -> Unit) {
    val c = Crumb.colors
    val spin = rememberInfiniteTransition(label = "locate")
    val angle by spin.animateFloat(0f, 360f, infiniteRepeatable(tween(900, easing = LinearEasing)), label = "angle")
    Row(
        Modifier
            .height(44.dp)
            .clip(ControlShape)
            .background(c.tint)
            .clickable(enabled = !locating, role = Role.Button, onClick = onClick)
            .padding(horizontal = 18.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            if (locating) Lucide.LoaderCircle else Lucide.LocateFixed,
            contentDescription = null,
            tint = c.primary,
            modifier = Modifier.size(18.dp).then(if (locating) Modifier.rotate(angle) else Modifier),
        )
        Text(
            if (saved) "Update my location" else "Use my location",
            color = c.primary,
            style = CrumbText.body.copy(fontWeight = FontWeight.Bold),
            maxLines = 1,
        )
    }
}

/** One live share link: title, meta, Copy and Unlink, with the inline stop confirmation. */
@Composable
private fun ShareRow(link: SharedLink, stopping: Boolean, onCopy: () -> Unit, onStop: () -> Unit) {
    val c = Crumb.colors
    var confirming by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxWidth().padding(12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            Icon(
                if (link.kind == "cookbook") Lucide.BookOpen else Lucide.CookingPot,
                contentDescription = null,
                tint = c.ink,
                modifier = Modifier.size(22.dp),
            )
            Column(Modifier.weight(1f)) {
                Text(link.title, style = CrumbText.rowTitle, color = c.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(shareMeta(link), style = CrumbText.bodySmall, color = c.inkMuted)
            }
            if (!confirming) {
                Btn(null, onCopy, style = BtnStyle.Ghost, icon = Lucide.Copy, contentDescription = "Copy the link to ${link.title}")
                Btn(null, { confirming = true }, style = BtnStyle.Ghost, icon = Lucide.Unlink, contentDescription = "Stop sharing ${link.title}")
            }
        }
        if (confirming) {
            Row(
                Modifier.fillMaxWidth().padding(start = 36.dp, top = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text(
                    "The link stops working for everyone.",
                    style = CrumbText.bodySmall,
                    color = c.inkMuted,
                    modifier = Modifier.weight(1f),
                )
                Btn("Keep", { confirming = false }, style = BtnStyle.Ghost)
                Btn("Stop sharing", onStop, style = BtnStyle.Danger, enabled = !stopping)
            }
        }
    }
}

private fun themeIcon(mode: ThemeMode): ImageVector = when (mode) {
    ThemeMode.Light -> Lucide.Sun
    ThemeMode.Dark -> Lucide.Moon
    ThemeMode.System -> Lucide.MonitorSmartphone
    ThemeMode.Sun -> Lucide.Sunrise
}

private fun roundOne(n: Double): Double = round(n * 10) / 10

/** Reads a coarse fix, as the web's `getCurrentPosition` does (with its 15s timeout). */
private object LocationReader {
    suspend fun current(context: Context): Location? {
        val manager = context.getSystemService(Context.LOCATION_SERVICE) as? LocationManager ?: return null
        val provider = listOf(LocationManager.NETWORK_PROVIDER, LocationManager.GPS_PROVIDER)
            .firstOrNull { runCatching { manager.isProviderEnabled(it) }.getOrDefault(false) }
        val fix = provider?.let { withTimeoutOrNull(15_000) { awaitFix(context, manager, it) } }
        return fix ?: lastKnown(manager, provider)
    }

    @SuppressLint("MissingPermission")
    private suspend fun awaitFix(context: Context, manager: LocationManager, provider: String): Location? =
        suspendCancellableCoroutine { cont ->
            val listener = LocationListener { location -> if (cont.isActive) cont.resume(location) }
            try {
                if (Build.VERSION.SDK_INT >= 30) {
                    manager.getCurrentLocation(provider, null, context.mainExecutor) { location ->
                        if (cont.isActive) cont.resume(location)
                    }
                } else {
                    manager.requestSingleUpdate(provider, listener, Looper.getMainLooper())
                }
            } catch (e: Exception) {
                if (cont.isActive) cont.resume(null)
            }
            cont.invokeOnCancellation { runCatching { manager.removeUpdates(listener) } }
        }

    @SuppressLint("MissingPermission")
    private fun lastKnown(manager: LocationManager, provider: String?): Location? =
        listOfNotNull(provider, LocationManager.NETWORK_PROVIDER, LocationManager.GPS_PROVIDER)
            .distinct()
            .mapNotNull { runCatching { manager.getLastKnownLocation(it) }.getOrNull() }
            .maxByOrNull { it.time }
}

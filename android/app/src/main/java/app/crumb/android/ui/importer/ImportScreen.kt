package app.crumb.android.ui.importer

import android.content.ContentResolver
import android.net.Uri
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
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import app.crumb.android.data.CreatedRecipe
import app.crumb.android.data.Importer
import app.crumb.android.data.Incoming
import app.crumb.android.data.ImportOutcome
import app.crumb.android.data.PhotoImport
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.CrumbNav
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.components.Card
import app.crumb.android.ui.components.Chip
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbInput
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.GroupLabel
import app.crumb.android.ui.components.ListCard
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.components.VSpace
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import app.crumb.core.bookImportedTitle
import app.crumb.core.linksInText
import com.composables.icons.lucide.ArrowLeft
import com.composables.icons.lucide.BookmarkCheck
import com.composables.icons.lucide.ChevronDown
import com.composables.icons.lucide.CircleCheck
import com.composables.icons.lucide.CircleX
import com.composables.icons.lucide.Clock
import com.composables.icons.lucide.Download
import com.composables.icons.lucide.FileUp
import com.composables.icons.lucide.LoaderCircle
import com.composables.icons.lucide.Lucide
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

enum class LinkState { Waiting, Working, Saved, Duplicate, Failed }

/** One link being imported, with where it ended up (a recipe or a shared cookbook). */
data class LinkJob(
    val url: String,
    val state: LinkState = LinkState.Waiting,
    val id: Long? = null,
    val title: String? = null,
    val cookbook: Boolean = false,
    val message: String? = null,
    /** Where a cooking video is in the server's queue ("Queued (2nd)…"), while it works. */
    val progress: String? = null,
)

enum class FileState { Working, Done, Failed }

/** One file (or a set of photo pages) being imported. */
data class FileJob(
    val id: Long,
    val name: String,
    val state: FileState,
    val created: List<CreatedRecipe> = emptyList(),
    val duplicates: Int = 0,
    val skipped: Int = 0,
    val message: String? = null,
    /** The recipe was read by OCR: open the editor so the amounts can be checked. */
    val edit: Boolean = false,
)

data class ImportUiState(
    val linksText: String = "",
    val jobs: List<LinkJob> = emptyList(),
    val running: Boolean = false,
    val fileJobs: List<FileJob> = emptyList(),
)

/**
 * Import recipes (web/src/islands/ImportTools.svelte): links scanned out of the text and
 * imported two at a time, and files or photo pages read by the server.
 */
class ImportViewModel(
    private val importer: Importer,
    private val resolver: ContentResolver,
    initialLinks: String?,
) : ViewModel() {
    private val _state = MutableStateFlow(ImportUiState(linksText = initialLinks.orEmpty()))
    val state: StateFlow<ImportUiState> = _state.asStateFlow()
    private var nextFileJob = 0L

    fun setLinks(text: String) {
        _state.update { it.copy(linksText = text) }
    }

    /** Two workers, as the web: fast enough and gentle on the server's headless browser. */
    fun runLinks() {
        if (_state.value.running) return
        val links = linksInText(_state.value.linksText)
        if (links.isEmpty()) return
        val jobs = links.map { LinkJob(it) }.toMutableList()
        _state.update { it.copy(jobs = jobs.toList(), running = true) }
        viewModelScope.launch {
            var cursor = 0
            suspend fun worker() {
                while (true) {
                    val i = cursor
                    if (i >= jobs.size) break
                    cursor++
                    jobs[i] = jobs[i].copy(state = LinkState.Working)
                    _state.update { it.copy(jobs = jobs.toList()) }
                    jobs[i] = try {
                        val outcome = importer.link(jobs[i].url) { line ->
                            jobs[i] = jobs[i].copy(progress = line)
                            _state.update { it.copy(jobs = jobs.toList()) }
                        }
                        when (outcome) {
                            is ImportOutcome.Saved -> jobs[i].copy(
                                progress = null,
                                state = if (outcome.isNew) LinkState.Saved else LinkState.Duplicate,
                                id = outcome.id,
                                title = outcome.title,
                            )
                            is ImportOutcome.Book -> jobs[i].copy(
                                state = if (outcome.cookbook.added > 0) LinkState.Saved else LinkState.Duplicate,
                                id = outcome.cookbook.id,
                                title = bookImportedTitle(
                                    outcome.cookbook.name,
                                    outcome.cookbook.added.toUInt(),
                                    outcome.cookbook.duplicates.toUInt(),
                                    outcome.cookbook.skipped.toUInt(),
                                ),
                                cookbook = true,
                            )
                            is ImportOutcome.Files -> jobs[i]
                        }
                    } catch (e: Exception) {
                        jobs[i].copy(state = LinkState.Failed, message = e.friendlyMessage(), progress = null)
                    }
                    _state.update { it.copy(jobs = jobs.toList()) }
                }
            }
            coroutineScope {
                launch { worker() }
                launch { worker() }
            }
            val done = _state.value.jobs
            val count = done.count { it.state == LinkState.Saved }
            _state.update { it.copy(running = false) }
            Toaster.show("Imported $count of ${done.size} links", tone = ToastTone.Success)
        }
    }

    /** Photos chosen together are the pages of one recipe; other files go one by one. */
    fun importPicked(uris: List<Uri>) {
        if (uris.isEmpty()) return
        val photos = uris.filter { Incoming.isPhoto(resolver.getType(it), Incoming.displayName(resolver, it)) }
        val files = uris.filterNot { it in photos }
        viewModelScope.launch {
            if (photos.isNotEmpty()) importPhotosNow(photos)
            if (files.isNotEmpty()) importFilesNow(files)
        }
    }

    private suspend fun importPhotosNow(photos: List<Uri>) {
        val firstName = Incoming.displayName(resolver, photos[0]) ?: "Photo"
        val name = photoJobName(photos.size, firstName)
        val id = nextFileJob++
        addFileJob(FileJob(id, name, FileState.Working))
        if (photos.size > PhotoImport.MAX_PHOTOS) {
            updateFileJob(id) { it.copy(state = FileState.Failed, message = tooManyPhotosText(PhotoImport.MAX_PHOTOS)) }
            return
        }
        try {
            val saved = withContext(Dispatchers.IO) {
                importer.photos(photos) { line -> updateFileJob(id) { it.copy(message = line) } }
            }
            updateFileJob(id) {
                it.copy(
                    state = FileState.Done,
                    created = if (saved.isNew) listOf(CreatedRecipe(saved.id, saved.title)) else emptyList(),
                    duplicates = if (saved.isNew) 0 else 1,
                    message = if (saved.onDevice) "Read on this device: check the amounts" else null,
                    edit = saved.onDevice,
                )
            }
        } catch (e: Exception) {
            updateFileJob(id) { it.copy(state = FileState.Failed, message = e.friendlyMessage()) }
        }
    }

    private suspend fun importFilesNow(files: List<Uri>) {
        val summaries = try {
            withContext(Dispatchers.IO) { importer.files(files) }
        } catch (e: Exception) {
            val name = files.firstOrNull()?.let { Incoming.displayName(resolver, it) } ?: "Files"
            addFileJob(FileJob(nextFileJob++, name, FileState.Failed, message = e.friendlyMessage()))
            return
        }
        if (summaries !is ImportOutcome.Files) return
        summaries.results.forEach { summary ->
            val failed = summary.error != null
            val job = FileJob(
                id = nextFileJob++,
                name = summary.file.ifBlank { "file" },
                state = if (failed) FileState.Failed else FileState.Done,
                created = summary.created,
                duplicates = summary.duplicates,
                skipped = summary.skipped,
                message = if (failed) summary.error ?: "Nothing imported" else null,
            )
            addFileJob(job)
        }
    }

    private fun addFileJob(job: FileJob) {
        _state.update { it.copy(fileJobs = listOf(job) + it.fileJobs) }
    }

    private fun updateFileJob(id: Long, transform: (FileJob) -> FileJob) {
        _state.update { s -> s.copy(fileJobs = s.fileJobs.map { if (it.id == id) transform(it) else it }) }
    }
}

/**
 * The Import recipes page (web/src/pages/import.astro + islands/ImportTools.svelte): links
 * pasted as any text, and files or photo pages handed to the server's importer.
 */
@Composable
fun ImportScreen(links: String?) {
    val container = AppContainerProvider
    val context = LocalContext.current
    val nav = LocalNav.current
    val vm: ImportViewModel = crumbViewModel {
        ImportViewModel(container.importer, context.contentResolver, links)
    }
    val state by vm.state.collectAsStateWithLifecycle()
    val c = Crumb.colors

    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        vm.importPicked(uris)
    }

    Column(
        Modifier.fillMaxSize().statusBarsPadding().imePadding().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp),
        verticalArrangement = Arrangement.spacedBy(28.dp),
    ) {
        VSpace(8.dp)
        BackLink { nav.back() }
        Column {
            Text("Import recipes", style = CrumbText.pageTitle, color = c.ink, modifier = Modifier.padding(top = 4.dp))
            Text(
                "Paste links or upload files. Duplicates are skipped.",
                style = CrumbText.body,
                color = c.inkMuted,
                modifier = Modifier.padding(top = 8.dp),
            )
        }

        LinksSection(state, vm)
        FilesSection(state, onPick = { picker.launch(ACCEPT) })
        FileJobs(state, onOpenRecipe = { id, edit -> if (edit) nav.edit(id) else nav.recipe(id) })
        JustTheRecipe(nav)
        VSpace(24.dp)
    }
}

@Composable
private fun LinksSection(state: ImportUiState, vm: ImportViewModel) {
    val c = Crumb.colors
    val count = linksInText(state.linksText).size
    Column {
        GroupLabel("Links")
        CrumbInput(
            value = state.linksText,
            onValueChange = vm::setLinks,
            minLines = 5,
            placeholder = "https://…\nhttps://…",
            modifier = Modifier.fillMaxWidth(),
        )
        Row(
            Modifier.fillMaxWidth().padding(top = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(linksFoundText(count), style = CrumbText.meta, color = c.inkMuted, modifier = Modifier.weight(1f))
            ImportLinksButton(count > 0 && !state.running, state.running, vm::runLinks)
        }
        if (state.jobs.isNotEmpty()) {
            VSpace(16.dp)
            ListCard(rows = state.jobs.map { job -> { LinkRow(job) } })
        }
    }
}

@Composable
private fun ImportLinksButton(enabled: Boolean, busy: Boolean, onClick: () -> Unit) {
    val c = Crumb.colors
    val spin = rememberInfiniteTransition(label = "import")
    val angle by spin.animateFloat(0f, 360f, infiniteRepeatable(tween(900, easing = LinearEasing)), label = "angle")
    val ink = if (enabled) c.onButter else c.inkMuted
    Row(
        Modifier
            .height(44.dp)
            .clip(ControlShape)
            .background(if (enabled) c.butter else c.tint)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 18.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            if (busy) Lucide.LoaderCircle else Lucide.Download,
            contentDescription = null,
            tint = ink,
            modifier = Modifier.size(18.dp).then(if (busy) Modifier.rotate(angle) else Modifier),
        )
        Text(
            if (busy) "Importing…" else "Import links",
            color = ink,
            fontFamily = NunitoSans,
            fontWeight = FontWeight.Bold,
            maxLines = 1,
        )
    }
}

@Composable
private fun LinkRow(job: LinkJob) {
    val c = Crumb.colors
    val nav = LocalNav.current
    Row(
        Modifier.fillMaxWidth().padding(12.dp),
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        StatusIcon(job.state == LinkState.Working, jobIcon(job.state), jobTint(job.state))
        Column(Modifier.weight(1f)) {
            if (job.id != null) {
                Text(
                    job.title.orEmpty(),
                    color = c.ink,
                    fontFamily = NunitoSans,
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.clickable {
                        if (job.cookbook) nav.cookbook(job.id) else nav.recipe(job.id)
                    },
                )
            }
            Text(job.url, style = CrumbText.bodySmall, color = c.inkMuted, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (job.state == LinkState.Duplicate && !job.cookbook) {
                Text("Already saved", style = CrumbText.meta, color = c.inkMuted)
            }
            job.progress?.let { Text(it, style = CrumbText.bodySmall.copy(fontSize = 13.sp), color = c.inkMuted) }
            job.message?.let { Text(it, style = CrumbText.bodySmall.copy(fontSize = 13.sp), color = c.error) }
        }
    }
}

@Composable
private fun StatusIcon(spinning: Boolean, icon: androidx.compose.ui.graphics.vector.ImageVector, tint: Color) {
    val spin = rememberInfiniteTransition(label = "spin")
    val angle by spin.animateFloat(0f, 360f, infiniteRepeatable(tween(900, easing = LinearEasing)), label = "angle")
    Icon(
        icon,
        contentDescription = null,
        tint = tint,
        modifier = Modifier.size(20.dp).then(if (spinning) Modifier.rotate(angle) else Modifier),
    )
}

private fun jobIcon(state: LinkState) = when (state) {
    LinkState.Waiting -> Lucide.Clock
    LinkState.Working -> Lucide.LoaderCircle
    LinkState.Saved -> Lucide.CircleCheck
    LinkState.Duplicate -> Lucide.BookmarkCheck
    LinkState.Failed -> Lucide.CircleX
}

@Composable
private fun jobTint(state: LinkState): Color {
    val c = Crumb.colors
    return when (state) {
        LinkState.Working -> c.primary
        LinkState.Saved -> c.primary
        LinkState.Failed -> c.error
        else -> c.inkMuted
    }
}

@Composable
private fun FilesSection(state: ImportUiState, onPick: () -> Unit) {
    val c = Crumb.colors
    Column {
        GroupLabel("Files")
        Card {
            Column(
                Modifier
                    .fillMaxWidth()
                    .dashedBorder(if (state.fileJobs.any { it.state == FileState.Working }) c.tile else c.lineStrong)
                    .clickable(onClick = onPick)
                    .padding(horizontal = 24.dp, vertical = 32.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Icon(Lucide.FileUp, contentDescription = null, tint = c.inkMuted, modifier = Modifier.size(24.dp))
                Text(
                    "Tap to choose",
                    color = c.ink,
                    fontFamily = NunitoSans,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(top = 6.dp),
                )
                Text(
                    "PDF, Paprika, Mealie JSON, web pages, text, .zip, a Crumb backup, or photos of a recipe",
                    style = CrumbText.meta,
                    color = c.inkMuted,
                    modifier = Modifier.padding(top = 2.dp),
                )
            }
        }
        Text(
            "Up to 50 MB each. In text files, put --- between recipes. Photos chosen together are read as the pages of one recipe (up to ${PhotoImport.MAX_PHOTOS}).",
            style = CrumbText.hint,
            color = c.inkMuted,
            modifier = Modifier.padding(top = 8.dp, start = 4.dp),
        )
    }
}

@Composable
private fun FileJobs(state: ImportUiState, onOpenRecipe: (Long, Boolean) -> Unit) {
    if (state.fileJobs.isEmpty()) return
    val c = Crumb.colors
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        state.fileJobs.forEach { job ->
            Card(Modifier.fillMaxWidth(), padding = PaddingValues(12.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    StatusIcon(
                        spinning = job.state == FileState.Working,
                        icon = when (job.state) {
                            FileState.Working -> Lucide.LoaderCircle
                            FileState.Done -> Lucide.CircleCheck
                            FileState.Failed -> Lucide.CircleX
                        },
                        tint = when (job.state) {
                            FileState.Failed -> c.error
                            FileState.Done -> c.primary
                            FileState.Working -> c.primary
                        },
                    )
                    Text(
                        job.name,
                        color = c.ink,
                        fontFamily = NunitoSans,
                        fontWeight = FontWeight.Bold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                    if (job.state == FileState.Done) {
                        Text(
                            fileResultText(job.created.size, job.duplicates, job.skipped),
                            style = CrumbText.bodySmall,
                            color = c.inkMuted,
                            maxLines = 1,
                        )
                    }
                }
                job.message?.let {
                    Text(
                        it,
                        style = CrumbText.bodySmall.copy(fontSize = 13.sp),
                        color = if (job.state == FileState.Failed) c.error else c.inkMuted,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                }
                if (job.created.isNotEmpty()) {
                    FlowRow(
                        Modifier.fillMaxWidth().padding(top = 10.dp),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        job.created.take(12).forEach { recipe ->
                            Chip(recipe.title, selected = false, onClick = { onOpenRecipe(recipe.id, job.edit) })
                        }
                    }
                    if (job.created.size > 12) {
                        Text(
                            "+${job.created.size - 12} more",
                            style = CrumbText.meta,
                            color = c.inkMuted,
                            modifier = Modifier.padding(top = 4.dp, start = 4.dp),
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun JustTheRecipe(nav: app.crumb.android.ui.CrumbNav) {
    val c = Crumb.colors
    var open by rememberSaveable { mutableStateOf(false) }
    Card(Modifier.fillMaxWidth()) {
        Row(
            Modifier
                .fillMaxWidth()
                .clickable { open = !open }
                .padding(horizontal = 16.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                "Coming from Just the Recipe?",
                color = c.ink,
                fontFamily = NunitoSans,
                fontWeight = FontWeight.Bold,
                modifier = Modifier.weight(1f),
            )
            Icon(Lucide.ChevronDown, contentDescription = null, tint = c.inkMuted, modifier = Modifier.size(20.dp).rotate(if (open) 180f else 0f))
        }
        if (open) {
            Text(
                buildAnnotatedString {
                    append("Paste each recipe's ")
                    withStyle(SpanStyle(fontStyle = FontStyle.Italic)) { append("Share") }
                    append(" link above, upload PDFs made with ")
                    withStyle(SpanStyle(fontStyle = FontStyle.Italic)) { append("Print → Save as PDF") }
                    append(", or paste the text on ")
                    withLink(LinkAnnotation.Clickable("add", TextLinkStyles(SpanStyle(color = c.primary))) { nav.add() }) {
                        append("Add")
                    }
                    append(".")
                },
                style = CrumbText.bodySmall,
                color = c.inkMuted,
                modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 14.dp),
            )
        }
    }
}

/** "← Back", 15sp muted. */
@Composable
private fun BackLink(onClick: () -> Unit) {
    val c = Crumb.colors
    Row(
        Modifier
            .clip(ControlShape)
            .clickable(onClick = onClick)
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Icon(Lucide.ArrowLeft, contentDescription = null, tint = c.inkMuted, modifier = Modifier.size(18.dp))
        Text("Back", style = CrumbText.bodySmall, color = c.inkMuted)
    }
}

/** A 2dp dashed border, like the web's drop zone. */
private fun Modifier.dashedBorder(color: Color) = drawBehind {
    val width = 2.dp.toPx()
    drawRoundRect(
        color = color,
        topLeft = Offset(width / 2, width / 2),
        size = Size(size.width - width, size.height - width),
        cornerRadius = CornerRadius(16.dp.toPx()),
        style = Stroke(width = width, pathEffect = PathEffect.dashPathEffect(floatArrayOf(10f, 8f), 0f)),
    )
}

private val ACCEPT = arrayOf(
    "application/pdf",
    "application/json",
    "application/zip",
    "application/octet-stream",
    "text/plain",
    "text/html",
    "text/markdown",
    "image/*",
)

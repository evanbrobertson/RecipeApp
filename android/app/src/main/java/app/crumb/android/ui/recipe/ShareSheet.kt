package app.crumb.android.ui.recipe

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.crumb.android.data.Cookbook
import app.crumb.android.data.CrumbJson
import app.crumb.android.data.Download
import app.crumb.android.data.Recipe
import app.crumb.android.data.Share
import app.crumb.android.data.ShareKind
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.ListCard
import app.crumb.android.ui.components.ListRow
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster
import app.crumb.android.ui.friendlyMessage
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import app.crumb.core.bookToText
import app.crumb.core.recipeToText
import com.composables.icons.lucide.Copy
import com.composables.icons.lucide.FileJson
import com.composables.icons.lucide.FileText
import com.composables.icons.lucide.Link
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Mail
import com.composables.icons.lucide.Share
import com.composables.icons.lucide.X
import kotlinx.coroutines.launch

/**
 * Share, send and download one recipe or one cookbook (web/src/components/ShareSheet.svelte):
 * its share link (made on request, with the notes on or off, stopped for good), plain text, a
 * mailto, the native share sheet, and files. A cookbook's link is live: recipes added later
 * show up on it. The sheet never creates a link on its own — the server has no GET for one,
 * so it offers "Create link" first.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ShareSheet(
    kind: ShareKind,
    id: Long,
    title: String,
    share: Share?,
    onShareChange: (Share?) -> Unit,
    onDismiss: () -> Unit,
    recipe: Recipe? = null,
    cookbook: Cookbook? = null,
) {
    val container = AppContainerProvider
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val c = Crumb.colors
    val book = kind == ShareKind.Cookbook
    val recipeJson = recipe?.let { CrumbJson.encodeToString(Recipe.serializer(), it) }
    val sheet = rememberModalBottomSheetState()
    var busy by remember { mutableStateOf(false) }
    var confirmStop by remember { mutableStateOf(false) }
    var pending by remember { mutableStateOf<Download?>(null) }

    val notesHint = when {
        book -> "Shown on each recipe's page"
        recipe?.notes?.isNotBlank() == true -> "Shown on the shared page"
        else -> "This recipe has no notes yet"
    }

    val jsonSaver = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")) { uri ->
        saveDownload(context, uri, pending) { pending = null }
    }
    val markdownSaver = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("text/markdown")) { uri ->
        saveDownload(context, uri, pending) { pending = null }
    }

    fun create() {
        scope.launch {
            busy = true
            try {
                onShareChange(container.api.createShare(kind, id))
            } catch (e: Exception) {
                Toaster.show("Couldn't make a link", e.friendlyMessage(), ToastTone.Error)
            } finally {
                busy = false
            }
        }
    }

    fun setNotes(include: Boolean) {
        val before = share ?: return
        onShareChange(before.copy(includeNotes = include))
        scope.launch {
            try {
                onShareChange(container.api.setShareNotes(kind, id, include))
            } catch (e: Exception) {
                onShareChange(before)
                Toaster.show("Couldn't change that", e.friendlyMessage(), ToastTone.Error)
            }
        }
    }

    fun stop() {
        scope.launch {
            busy = true
            try {
                container.api.stopSharing(kind, id)
                onShareChange(null)
                confirmStop = false
                Toaster.show("Stopped sharing", "The link no longer works.")
            } catch (e: Exception) {
                Toaster.show("Couldn't stop sharing", e.friendlyMessage(), ToastTone.Error)
            } finally {
                busy = false
            }
        }
    }

    fun copyText() {
        val text = if (book) {
            bookToText(title, cookbook?.recipes?.map { it.title }.orEmpty(), share?.url)
        } else {
            recipeToText(recipeJson.orEmpty(), share?.url)
        }
        copyToClipboard(context, title, text, if (book) "Cookbook copied" else "Recipe copied")
    }

    fun mail() {
        val link = share ?: return
        val intent = Intent(Intent.ACTION_SENDTO).apply {
            data = Uri.parse("mailto:?subject=${Uri.encode(title)}&body=${Uri.encode(link.url)}")
        }
        runCatching { context.startActivity(intent) }
    }

    fun shareAsText() = sendText(context, recipeToText(recipeJson.orEmpty(), share?.url))

    fun export(format: String) {
        scope.launch {
            try {
                val download = if (book) container.api.exportCookbook(id) else container.api.exportRecipe(id, format)
                pending = download
                if (format == "md" && !book) markdownSaver.launch(download.fileName) else jsonSaver.launch(download.fileName)
            } catch (e: Exception) {
                Toaster.show("Couldn't download that", e.friendlyMessage(), ToastTone.Error)
            }
        }
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheet,
        containerColor = c.paper,
        shape = RoundedCornerShape(topStart = 16.dp, topEnd = 16.dp),
        dragHandle = {
            Box(Modifier.fillMaxWidth().padding(vertical = 10.dp), contentAlignment = Alignment.Center) {
                Box(Modifier.width(40.dp).height(4.dp).clip(CircleShape).background(c.lineStrong))
            }
        },
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .navigationBarsPadding()
                .padding(bottom = 24.dp),
        ) {
            Row(Modifier.fillMaxWidth().padding(bottom = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                Text("Share", style = CrumbText.sectionTitle, color = c.ink, modifier = Modifier.weight(1f))
                Btn(null, onDismiss, style = BtnStyle.Ghost, icon = Lucide.X, contentDescription = "Close")
            }

            GroupTitle("Share a link")
            if (share == null) {
                Text(
                    if (book) "Anyone with the link sees this book and every recipe in it, including ones you add later."
                    else "Anyone with the link can see this recipe, not the rest of your box.",
                    style = CrumbText.body,
                    color = c.inkMuted,
                )
                if (book) {
                    Text(
                        "Links to single recipes in this book open the whole book.",
                        style = CrumbText.meta,
                        color = c.inkMuted,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                }
                Btn("Create link", ::create, style = BtnStyle.Soft, icon = Lucide.Link, enabled = !busy, modifier = Modifier.padding(top = 12.dp))
            } else {
                ReadOnlyField(share.url)
                if (book) {
                    Text(
                        "Links to single recipes in this book open the whole book.",
                        style = CrumbText.meta,
                        color = c.inkMuted,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                Row(Modifier.padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Btn("Copy link", { copyToClipboard(context, "Crumb link", share.url, "Link copied") }, style = BtnStyle.Soft, icon = Lucide.Copy)
                    Btn("Share…", { sendText(context, share.url) }, style = BtnStyle.Soft, icon = Lucide.Share)
                }
                NotesToggle(share.includeNotes, notesHint, onCheckedChange = ::setNotes)
                if (confirmStop) {
                    Column(Modifier.fillMaxWidth().padding(top = 16.dp).clip(ControlShape).background(c.tint).padding(16.dp)) {
                        Text("Stop sharing?", style = CrumbText.label, color = c.ink)
                        Text(
                            "The link stops working for everyone. A new link can be made later.",
                            style = CrumbText.bodySmall,
                            color = c.inkMuted,
                            modifier = Modifier.padding(top = 4.dp),
                        )
                        Row(Modifier.fillMaxWidth().padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                            Btn("Keep sharing", { confirmStop = false }, style = BtnStyle.Ghost)
                            Btn("Stop sharing", ::stop, style = BtnStyle.Danger, enabled = !busy)
                        }
                    }
                } else {
                    Btn("Stop sharing", { confirmStop = true }, style = BtnStyle.Link, modifier = Modifier.padding(top = 4.dp))
                }
            }

            Spacer(Modifier.height(28.dp))
            GroupTitle("Send")
            val sendRows = mutableListOf<@Composable () -> Unit>()
            sendRows += { ListRow(title = "Copy as text", plainIcon = true, icon = Lucide.Copy, chevron = false, onClick = ::copyText) }
            if (share != null) sendRows += { ListRow(title = "Email", plainIcon = true, icon = Lucide.Mail, chevron = false, onClick = ::mail) }
            if (!book) sendRows += { ListRow(title = "Share as text", plainIcon = true, icon = Lucide.Share, chevron = false, onClick = ::shareAsText) }
            ListCard(rows = sendRows)

            Spacer(Modifier.height(28.dp))
            GroupTitle("Download")
            val downloadRows = mutableListOf<@Composable () -> Unit>()
            downloadRows += { ListRow(title = "For another Crumb (.json)", plainIcon = true, icon = Lucide.FileJson, chevron = false, onClick = { export("json") }) }
            if (!book) downloadRows += { ListRow(title = "Markdown (.md)", plainIcon = true, icon = Lucide.FileText, chevron = false, onClick = { export("md") }) }
            ListCard(rows = downloadRows)
        }
    }
}

/** The web's uppercase `.group-title`, without the screen's extra start padding. */
@Composable
private fun GroupTitle(text: String) {
    Text(
        text.uppercase(),
        style = CrumbText.kicker,
        color = Crumb.colors.inkMuted,
        modifier = Modifier.padding(bottom = 10.dp),
    )
}

/** The link once it exists: a read-only field the server named. */
@Composable
private fun ReadOnlyField(url: String) {
    val c = Crumb.colors
    Box(
        Modifier
            .fillMaxWidth()
            .clip(ControlShape)
            .background(c.paper)
            .border(1.dp, c.lineStrong, ControlShape)
            .heightIn(min = 44.dp)
            .padding(horizontal = 14.dp, vertical = 12.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        Text(url, color = c.ink, fontFamily = NunitoSans, fontSize = 15.sp)
    }
}

/** The web's on/off switch: a 44×26 track, tile when on, the hint under the label. */
@Composable
private fun NotesToggle(checked: Boolean, hint: String, onCheckedChange: (Boolean) -> Unit) {
    val c = Crumb.colors
    Row(
        Modifier
            .fillMaxWidth()
            .padding(top = 16.dp)
            .toggleable(value = checked, role = Role.Switch, onValueChange = onCheckedChange),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text("Include my notes", color = c.ink, fontFamily = NunitoSans, fontSize = 16.sp, fontWeight = FontWeight.Bold)
            Text(hint, style = CrumbText.bodySmall, color = c.inkMuted)
        }
        Box(Modifier.width(44.dp).height(26.dp).clip(CircleShape).background(if (checked) c.tile else c.lineStrong)) {
            Box(
                Modifier
                    .align(if (checked) Alignment.CenterEnd else Alignment.CenterStart)
                    .padding(3.dp)
                    .size(20.dp)
                    .clip(CircleShape)
                    .background(c.paper),
            )
        }
    }
}

/** Copy [text] to the clipboard and say so (web's clipboard.writeText + toast). */
private fun copyToClipboard(context: Context, label: String, text: String, done: String) {
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    clipboard.setPrimaryClip(ClipData.newPlainText(label, text))
    Toaster.show(done)
}

/** The native share sheet (Intent.ACTION_SEND text/plain). */
private fun sendText(context: Context, text: String) {
    val intent = Intent(Intent.ACTION_SEND).apply {
        type = "text/plain"
        putExtra(Intent.EXTRA_TEXT, text)
    }
    runCatching { context.startActivity(Intent.createChooser(intent, null)) }
}

/** Write the downloaded bytes to the file the picker chose, then say "Saved". */
private fun saveDownload(context: Context, uri: Uri?, download: Download?, clear: () -> Unit) {
    if (uri == null || download == null) {
        clear()
        return
    }
    try {
        context.contentResolver.openOutputStream(uri)?.use { it.write(download.bytes) }
        Toaster.show("Saved", tone = ToastTone.Success)
    } catch (e: Exception) {
        Toaster.show("Couldn't save that", e.message ?: "Try again.", ToastTone.Error)
    } finally {
        clear()
    }
}

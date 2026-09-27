package app.crumb.android.ui.connect

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.crumb.android.data.ConnectorInfo
import app.crumb.android.data.CrumbApi
import app.crumb.android.data.Loaded
import app.crumb.android.ui.AppContainerProvider
import app.crumb.android.ui.LoadingViewModel
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.UiState
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.Card
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.FieldLabel
import app.crumb.android.ui.components.GroupLabel
import app.crumb.android.ui.components.ListCard
import app.crumb.android.ui.components.Message
import app.crumb.android.ui.components.Skeleton
import app.crumb.android.ui.components.VSpace
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import com.composables.icons.lucide.ArrowLeft
import com.composables.icons.lucide.Check
import com.composables.icons.lucide.Copy
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.ShieldAlert
import kotlinx.coroutines.delay

class ConnectViewModel(api: CrumbApi) : LoadingViewModel<ConnectorInfo>() {
    private val api = api

    init {
        load()
    }

    override suspend fun fetch(): Loaded<ConnectorInfo> = Loaded(api.connector())
}

/**
 * Connect to Claude (web/src/pages/connect.astro + islands/ConnectorUrl.svelte): the MCP URL
 * with its copy button, the Claude app steps, the Claude Code command and some things to ask.
 */
@Composable
fun ConnectScreen() {
    val container = AppContainerProvider
    val nav = LocalNav.current
    val vm: ConnectViewModel = crumbViewModel { ConnectViewModel(container.api) }
    val state by vm.state.collectAsStateWithLifecycle()

    LaunchedEffect(state) {
        if ((state as? UiState.Failed)?.signedOut == true) nav.signedOut()
    }

    Column(
        Modifier.fillMaxSize().statusBarsPadding().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp),
        verticalArrangement = Arrangement.spacedBy(28.dp),
    ) {
        VSpace(8.dp)
        BackLink { nav.back() }
        when (val current = state) {
            is UiState.Loading -> Skeleton(Modifier.fillMaxWidth().size(112.dp))
            is UiState.Failed -> Message(
                "Couldn't load the connector",
                current.message,
                action = { Btn("Try again", { vm.load() }, style = BtnStyle.Outline) },
            )
            is UiState.Ready -> Connector(current.value)
        }
        VSpace(24.dp)
    }
}

@Composable
private fun Connector(info: ConnectorInfo) {
    val c = Crumb.colors
    Column(verticalArrangement = Arrangement.spacedBy(28.dp)) {
        Column {
            Text("Connect to Claude", style = CrumbText.pageTitle, color = c.ink, modifier = Modifier.padding(top = 4.dp))
            Text(
                "Save, search and tweak your recipes from any Claude chat.",
                style = CrumbText.body,
                color = c.inkMuted,
                modifier = Modifier.padding(top = 8.dp),
            )
        }

        ConnectorUrlCard(info)

        Column {
            GroupLabel("Claude app")
            ListCard(
                rows = listOf<@Composable () -> Unit>(
                    { StepRow(1, "In Claude, open ", "Settings → Connectors", ".") },
                    { StepRow(2, "Choose ", "Add custom connector", ", name it \u201CCrumb\u201D and paste the URL.") },
                    { StepRow(3, "Click ", "Connect", " and approve it with your app password.") },
                    { StepRowPlain(4, "In a chat, switch the connector on in the tools menu.") },
                ),
            )
            Text(
                "Works in the mobile apps too. On Team and Enterprise plans an owner may need to add it first.",
                style = CrumbText.hint,
                color = c.inkMuted,
                modifier = Modifier.padding(top = 8.dp, start = 4.dp),
            )
        }

        Column {
            GroupLabel("Claude Code")
            CodeBlock(claudeCommand(info.mcpUrl))
            Text(
                buildAnnotatedString {
                    append("Then run ")
                    withStyle(SpanStyle(fontFamily = FontFamily.Monospace, fontWeight = FontWeight.Bold, color = c.ink)) {
                        append("/mcp")
                    }
                    append(" to sign in.")
                },
                style = CrumbText.hint,
                color = c.inkMuted,
                modifier = Modifier.padding(top = 8.dp, start = 4.dp),
            )
        }

        Column {
            GroupLabel("Try asking")
            ListCard(
                rows = listOf(
                    { AskRow("Save this recipe to my recipe box: …") },
                    { AskRow("What can I make with chicken thighs and lemons?") },
                    { AskRow("Double my banana bread and save it.") },
                ),
            )
        }
    }
}

@Composable
private fun ConnectorUrlCard(info: ConnectorInfo) {
    val c = Crumb.colors
    val clipboard = LocalClipboardManager.current
    var copied by remember { mutableStateOf(false) }
    LaunchedEffect(copied) {
        if (copied) {
            delay(2000)
            copied = false
        }
    }
    Card(Modifier.fillMaxWidth(), padding = androidx.compose.foundation.layout.PaddingValues(16.dp)) {
        FieldLabel("Connector URL")
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            BoxedUrl(info.mcpUrl, Modifier.weight(1f))
            Btn(
                if (copied) "Copied" else "Copy",
                onClick = {
                    clipboard.setText(AnnotatedString(info.mcpUrl))
                    copied = true
                },
                style = BtnStyle.Primary,
                icon = if (copied) Lucide.Check else Lucide.Copy,
            )
        }
        if (!info.authEnabled) {
            Row(Modifier.fillMaxWidth().padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Icon(Lucide.ShieldAlert, contentDescription = null, tint = c.error, modifier = Modifier.size(16.dp))
                Text(
                    buildAnnotatedString {
                        withStyle(SpanStyle(fontWeight = FontWeight.Bold, color = c.ink)) { append("No password set.") }
                        append(" Anyone with this URL can change your recipes. Set APP_PASSWORD before you deploy.")
                    },
                    style = CrumbText.bodySmall,
                    color = c.inkMuted,
                )
            }
        }
    }
}

@Composable
private fun BoxedUrl(url: String, modifier: Modifier = Modifier) {
    val c = Crumb.colors
    Row(
        modifier
            .heightIn(min = 44.dp)
            .clip(ControlShape)
            .background(c.paper)
            .border(1.dp, c.lineStrong, ControlShape)
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            url,
            color = c.ink,
            fontFamily = FontFamily.Monospace,
            fontSize = 14.sp,
            maxLines = 1,
        )
    }
}

@Composable
private fun CodeBlock(command: String) {
    val c = Crumb.colors
    val clipboard = LocalClipboardManager.current
    var copied by remember { mutableStateOf(false) }
    LaunchedEffect(copied) {
        if (copied) {
            delay(2000)
            copied = false
        }
    }
    Row(
        Modifier
            .fillMaxWidth()
            .clip(ControlShape)
            .background(c.tint)
            .padding(start = 16.dp, top = 12.dp, bottom = 12.dp, end = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            command,
            color = c.ink,
            fontFamily = FontFamily.Monospace,
            fontSize = 14.sp,
            modifier = Modifier.weight(1f),
        )
        Btn(
            null,
            onClick = {
                clipboard.setText(AnnotatedString(command))
                copied = true
            },
            style = BtnStyle.Ghost,
            icon = if (copied) Lucide.Check else Lucide.Copy,
            contentDescription = "Copy the command",
        )
    }
}

@Composable
private fun StepRow(number: Int, before: String, bold: String, after: String) {
    val c = Crumb.colors
    StepRowShell(number) {
        Text(
            buildAnnotatedString {
                append(before)
                withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { append(bold) }
                append(after)
            },
            style = CrumbText.body,
            color = c.ink,
        )
    }
}

@Composable
private fun StepRowPlain(number: Int, text: String) {
    val c = Crumb.colors
    StepRowShell(number) { Text(text, style = CrumbText.body, color = c.ink) }
}

@Composable
private fun StepRowShell(number: Int, content: @Composable () -> Unit) {
    val c = Crumb.colors
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            number.toString(),
            color = c.inkMuted,
            fontFamily = NunitoSans,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.size(20.dp),
        )
        content()
    }
}

@Composable
private fun AskRow(text: String) {
    val c = Crumb.colors
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("\u201C$text\u201D", style = CrumbText.body.copy(fontSize = 15.sp), color = c.ink)
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

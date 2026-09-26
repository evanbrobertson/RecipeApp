package app.crumb.android.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.X

/**
 * The web's Modal (components/Modal.svelte): a paper card with a serif title, an optional
 * muted description, a ghost close button, the body, and footer buttons on the right.
 * Dismissed by the close button, back, or a tap on the wash.
 */
@Composable
fun CrumbModal(
    title: String,
    onDismiss: () -> Unit,
    description: String? = null,
    footer: (@Composable RowScope.() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val c = Crumb.colors
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(
            Modifier
                .padding(16.dp)
                .widthIn(max = 448.dp)
                .fillMaxWidth()
                .imePadding()
                .shadow(28.dp, CardShape, ambientColor = c.ink, spotColor = c.ink)
                .clip(CardShape)
                .background(c.paper)
                .border(1.dp, c.line, CardShape)
                .verticalScroll(rememberScrollState())
                .padding(20.dp),
        ) {
            Row(verticalAlignment = Alignment.Top) {
                Column(Modifier.weight(1f)) {
                    Text(title, style = CrumbText.sectionTitle, color = c.ink)
                    if (description != null) {
                        Text(description, style = CrumbText.bodySmall, color = c.inkMuted, modifier = Modifier.padding(top = 6.dp))
                    }
                }
                Btn(null, onDismiss, style = BtnStyle.Ghost, icon = Lucide.X, contentDescription = "Close",
                    modifier = Modifier.padding(start = 12.dp).offset(x = 10.dp, y = (-6).dp))
            }
            Column(Modifier.padding(top = 16.dp), content = content)
            if (footer != null) {
                FlowRow(
                    Modifier.fillMaxWidth().padding(top = 24.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) { Row(horizontalArrangement = Arrangement.spacedBy(8.dp), content = footer) }
            }
        }
    }
}

/** `.label` above a field. */
@Composable
fun FieldLabel(text: String, modifier: Modifier = Modifier) {
    Text(text, style = CrumbText.label, color = Crumb.colors.ink, modifier = modifier.padding(bottom = 6.dp))
}

/**
 * `.input`: paper, a strong 1dp line that turns tile green with a tint ring when focused,
 * 12dp corners, 16sp text, at least 44dp tall ([minLines] > 1 for a textarea).
 */
@Composable
fun CrumbInput(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    placeholder: String? = null,
    minLines: Int = 1,
    singleLine: Boolean = minLines == 1,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
    textStyle: TextStyle = TextStyle(fontFamily = NunitoSans, fontSize = 16.sp, lineHeight = 22.sp),
) {
    val c = Crumb.colors
    var focused by remember { mutableStateOf(false) }
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        singleLine = singleLine,
        minLines = minLines,
        keyboardOptions = keyboardOptions,
        keyboardActions = keyboardActions,
        textStyle = textStyle.copy(color = c.ink),
        cursorBrush = SolidColor(c.tile),
        modifier = modifier
            .fillMaxWidth()
            .onFocusChanged { focused = it.isFocused },
        decorationBox = { inner ->
            Box(
                Modifier
                    .then(if (focused) Modifier.border(3.dp, c.tint, ControlShape).padding(0.dp) else Modifier)
                    .clip(ControlShape)
                    .background(c.paper)
                    .border(1.dp, if (focused) c.tile else c.lineStrong, ControlShape)
                    .heightIn(min = if (minLines > 1) 80.dp else 44.dp)
                    .padding(horizontal = 14.dp, vertical = 10.dp),
                contentAlignment = if (minLines > 1) Alignment.TopStart else Alignment.CenterStart,
            ) {
                if (value.isEmpty() && placeholder != null) {
                    Text(placeholder, style = textStyle, color = c.inkMuted)
                }
                inner()
            }
        },
    )
}

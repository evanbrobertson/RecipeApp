package app.crumb.android.ui.cook

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnSize
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.CardShape
import app.crumb.android.ui.components.Chip
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.theme.Crumb
import app.crumb.android.ui.theme.NunitoSans
import app.crumb.core.StepTimer
import app.crumb.core.nudgeTimer
import app.crumb.core.timerDefault
import app.crumb.core.timerFromMinutes
import app.crumb.core.timerMax
import app.crumb.core.timerMin
import app.crumb.core.timerWords
import com.composables.icons.lucide.AlarmClock
import com.composables.icons.lucide.AlarmClockPlus
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Minus
import com.composables.icons.lucide.Plus

/**
 * A step's timer (web/src/components/StepTimer.svelte): suggested from the times the step
 * mentions, never trusted. The cook nudges or types the length before starting it, and any
 * step can have one. [seconds] is the length the cook chose, if they changed it; [onSeconds]
 * remembers a change.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun StepTimerControl(
    choices: List<StepTimer>,
    seconds: Int?,
    onSeconds: (Int) -> Unit,
    onStart: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = Crumb.colors
    val length = timerLength(seconds, choices.firstOrNull()?.seconds?.toInt())

    if (length == null) {
        Btn(
            "Set a timer",
            { onSeconds(timerDefault().toInt()) },
            modifier = modifier,
            style = BtnStyle.Soft,
            size = BtnSize.Lg,
            icon = Lucide.AlarmClockPlus,
        )
        return
    }

    val min = timerMin().toInt()
    val max = timerMax().toInt()
    var typing by remember { mutableStateOf(false) }

    Column(modifier) {
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
            itemVerticalAlignment = Alignment.CenterVertically,
        ) {
            Row(
                Modifier.clip(CardShape).background(c.tint).padding(4.dp),
                horizontalArrangement = Arrangement.spacedBy(2.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Btn(
                    text = null,
                    onClick = { onSeconds(nudgeTimer(length.toUInt(), false).toInt()) },
                    style = BtnStyle.Ghost,
                    icon = Lucide.Minus,
                    enabled = length > min,
                    contentDescription = "Shorter",
                )
                if (typing) {
                    MinutesField(
                        length,
                        onDone = { typed ->
                            typed?.let(::timerFromMinutes)?.let { onSeconds(it.toInt()) }
                            typing = false
                        },
                    )
                } else {
                    val words = timerWords(length.toUInt())
                    Box(
                        Modifier
                            .height(44.dp)
                            .defaultMinSize(minWidth = 80.dp)
                            .clip(ControlShape)
                            .clickable(role = Role.Button) { typing = true }
                            .semantics { contentDescription = "$words. Tap to type a length" }
                            .padding(horizontal = 8.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            words,
                            style = CrumbText.body.copy(
                                fontSize = 18.sp,
                                fontWeight = FontWeight.Bold,
                                fontFeatureSettings = "tnum",
                            ),
                            color = c.ink,
                        )
                    }
                }
                Btn(
                    text = null,
                    onClick = { onSeconds(nudgeTimer(length.toUInt(), true).toInt()) },
                    style = BtnStyle.Ghost,
                    icon = Lucide.Plus,
                    enabled = length < max,
                    contentDescription = "Longer",
                )
            }
            Btn(
                "Start timer",
                { onStart(length) },
                style = BtnStyle.Tile,
                size = BtnSize.Lg,
                icon = Lucide.AlarmClock,
            )
        }

        if (choices.size > 1) {
            FlowRow(
                Modifier.padding(top = 12.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
                itemVerticalAlignment = Alignment.CenterVertically,
            ) {
                Text("The step mentions", style = CrumbText.meta, color = c.inkMuted)
                choices.forEach { t ->
                    Chip(t.label, selected = length == t.seconds.toInt(), onClick = { onSeconds(t.seconds.toInt()) })
                }
            }
        }
    }
}

/**
 * Tapping the length types it, in minutes: a decimal keyboard, confirmed by Done or by leaving
 * the field. [onDone] gets what was typed, or null for nothing usable.
 */
@Composable
private fun MinutesField(length: Int, onDone: (Double?) -> Unit) {
    val c = Crumb.colors
    val text = minutesText(length)
    var value by remember { mutableStateOf(TextFieldValue(text, TextRange(0, text.length))) }
    val focus = remember { FocusRequester() }
    var focused by remember { mutableStateOf(false) }
    var finished by remember { mutableStateOf(false) }
    fun finish() {
        if (finished) return
        finished = true
        onDone(typedMinutes(value.text))
    }

    LaunchedEffect(Unit) { focus.requestFocus() }
    Row(Modifier.height(44.dp).padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        BasicTextField(
            value = value,
            onValueChange = { value = it },
            singleLine = true,
            textStyle = TextStyle(
                fontFamily = NunitoSans,
                fontSize = 18.sp,
                fontWeight = FontWeight.Bold,
                color = c.ink,
                textAlign = TextAlign.Center,
            ),
            cursorBrush = androidx.compose.ui.graphics.SolidColor(c.ink),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { finish() }),
            modifier = Modifier
                .width(80.dp)
                .height(40.dp)
                .clip(ControlShape)
                .background(c.paper)
                .focusRequester(focus)
                .onFocusChanged {
                    if (it.isFocused) focused = true else if (focused) finish()
                }
                .semantics { contentDescription = "Timer length in minutes" },
            decorationBox = { inner ->
                Box(Modifier.padding(horizontal = 8.dp), contentAlignment = Alignment.Center) { inner() }
            },
        )
        Text("min", style = CrumbText.body.copy(fontSize = 18.sp, fontWeight = FontWeight.Bold), color = c.ink, modifier = Modifier.padding(start = 6.dp))
    }
}

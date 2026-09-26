package app.crumb.android.ui.edit

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
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
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties
import app.crumb.android.data.Flag
import app.crumb.android.data.RecipeFields
import app.crumb.android.ui.components.Btn
import app.crumb.android.ui.components.BtnSize
import app.crumb.android.ui.components.BtnStyle
import app.crumb.android.ui.components.Card
import app.crumb.android.ui.components.CardShape
import app.crumb.android.ui.components.ControlShape
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.HSpace
import app.crumb.android.ui.components.VSpace
import app.crumb.android.ui.components.CrumbInput
import app.crumb.android.ui.theme.Crumb
import app.crumb.core.categories
import app.crumb.core.reviewText
import com.composables.icons.lucide.Check
import com.composables.icons.lucide.ChefHat
import com.composables.icons.lucide.ChevronDown
import com.composables.icons.lucide.LoaderCircle
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Plus
import com.composables.icons.lucide.Trash2

/**
 * The recipe editor (web/src/components/RecipeEditor.svelte, spec §7.3). Sections are edited as
 * one field per line; the title is the only required field. Wee Chef's review flags show inline
 * under the section that still has the line, each with a one-tap fix and "Keep as is".
 */
@Composable
fun RecipeEditor(
    initial: RecipeDraft,
    saving: Boolean,
    onSubmit: (RecipeFields) -> Unit,
    onCancel: () -> Unit,
    submitLabel: String = "Save",
    flags: List<Flag> = emptyList(),
    onDismissFlag: ((Flag) -> Unit)? = null,
) {
    val c = Crumb.colors
    var draft by remember { mutableStateOf(initial) }
    var error by remember { mutableStateOf<String?>(null) }

    fun submit() {
        error = null
        if (draft.title.trim().isEmpty()) {
            error = "Give the recipe a title."
            return
        }
        onSubmit(draft.toFields())
    }

    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(28.dp)) {
        Card(Modifier.fillMaxWidth(), padding = PaddingValues(16.dp)) {
            EditorLabel("Title", required = true)
            CrumbInput(
                draft.title,
                { draft = draft.copy(title = it) },
                modifier = Modifier.height(56.dp),
                placeholder = "Grandma's lasagne",
            )
            error?.let {
                Text(
                    it,
                    style = CrumbText.bodySmall.copy(fontWeight = FontWeight.Bold),
                    color = c.error,
                    modifier = Modifier.padding(top = 6.dp),
                )
            }
            VSpace(16.dp)
            EditorLabel("Description")
            CrumbInput(draft.description, { draft = draft.copy(description = it) }, minLines = 2)
        }

        Column {
            Text("Details", style = CrumbText.sectionTitle, color = c.ink, modifier = Modifier.padding(bottom = 14.dp))
            Card(Modifier.fillMaxWidth(), padding = PaddingValues(16.dp)) {
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    DetailCell("Prep time", Modifier.weight(1f)) {
                        CrumbInput(draft.prepTime, { draft = draft.copy(prepTime = it) }, placeholder = "15m")
                    }
                    DetailCell("Cook time", Modifier.weight(1f)) {
                        CrumbInput(draft.cookTime, { draft = draft.copy(cookTime = it) }, placeholder = "30m")
                    }
                }
                VSpace(16.dp)
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    DetailCell("Extra time", Modifier.weight(1f)) {
                        CrumbInput(draft.freezeTime, { draft = draft.copy(freezeTime = it) }, placeholder = "Chill 1h")
                    }
                    DetailCell("Total time", Modifier.weight(1f)) {
                        CrumbInput(draft.totalTime, { draft = draft.copy(totalTime = it) }, placeholder = "1h 45m")
                    }
                }
                VSpace(16.dp)
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    DetailCell("Yield", Modifier.weight(1f)) {
                        CrumbInput(draft.recipeYield, { draft = draft.copy(recipeYield = it) }, placeholder = "4 servings")
                    }
                    DetailCell("Category", Modifier.weight(1f)) {
                        CategoryField(draft.recipeCategory) { draft = draft.copy(recipeCategory = it) }
                    }
                }
                VSpace(16.dp)
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    DetailCell("Cuisine", Modifier.weight(1f)) {
                        CrumbInput(draft.recipeCuisine, { draft = draft.copy(recipeCuisine = it) }, placeholder = "Italian")
                    }
                    DetailCell("Author", Modifier.weight(1f)) {
                        CrumbInput(draft.author, { draft = draft.copy(author = it) })
                    }
                }
            }
        }

        SectionEditor("ingredients", draft, { draft = it }, flags, onDismissFlag)
        SectionEditor("instructions", draft, { draft = it }, flags, onDismissFlag)

        Column {
            Text("Notes and sources", style = CrumbText.sectionTitle, color = c.ink, modifier = Modifier.padding(bottom = 14.dp))
            Card(Modifier.fillMaxWidth(), padding = PaddingValues(16.dp)) {
                EditorLabel("Notes")
                // The cook's own notes, in their own hand (as on the recipe page)
                CrumbInput(
                    draft.notes,
                    { draft = draft.copy(notes = it) },
                    minLines = 3,
                    placeholder = "Less sugar next time…",
                    textStyle = CrumbText.hand.copy(fontSize = 24.sp, lineHeight = 30.sp),
                )
                VSpace(16.dp)
                EditorLabel("Nutrition")
                CrumbInput(draft.nutrition, { draft = draft.copy(nutrition = it) }, minLines = 3)
                Text(
                    "One per line, e.g. calories: 320",
                    style = CrumbText.hint,
                    color = c.inkMuted,
                    modifier = Modifier.padding(top = 6.dp),
                )
                VSpace(16.dp)
                EditorLabel("Source link")
                CrumbInput(draft.url, { draft = draft.copy(url = it) }, placeholder = "https://…")
                VSpace(16.dp)
                EditorLabel("Image link")
                CrumbInput(draft.image, { draft = draft.copy(image = it) }, placeholder = "https://…")
            }
        }

        Column(Modifier.fillMaxWidth()) {
            HorizontalDivider(color = c.line)
            Row(
                Modifier.fillMaxWidth().padding(top = 20.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Btn("Cancel", onCancel, style = BtnStyle.Ghost, size = BtnSize.Lg)
                Btn(
                    submitLabel,
                    { submit() },
                    style = BtnStyle.Primary,
                    size = BtnSize.Lg,
                    icon = if (saving) Lucide.LoaderCircle else Lucide.Check,
                    enabled = !saving,
                )
            }
        }
    }
}

/** `.label`, with the red `*` the web puts on the required title. */
@Composable
private fun EditorLabel(text: String, modifier: Modifier = Modifier, required: Boolean = false) {
    val c = Crumb.colors
    Row(modifier.padding(bottom = 6.dp)) {
        Text(text, style = CrumbText.label, color = c.ink)
        if (required) {
            HSpace(4.dp)
            Text("*", style = CrumbText.label, color = c.error)
        }
    }
}

/** A details-grid cell: the label above its field. */
@Composable
private fun DetailCell(label: String, modifier: Modifier = Modifier, field: @Composable () -> Unit) {
    Column(modifier) {
        EditorLabel(label)
        field()
    }
}

/** The Category `<select>` (spec §7.3): None, a legacy "(old) X" while it's selected, then the fixed list. */
@Composable
private fun CategoryField(value: String, onChange: (String) -> Unit) {
    val c = Crumb.colors
    val categories = remember { categories() }
    val legacy = legacyCategory(value, categories)
    var open by remember { mutableStateOf(false) }
    Box {
        Row(
            Modifier
                .fillMaxWidth()
                .height(44.dp)
                .clip(ControlShape)
                .background(c.paper)
                .border(1.dp, c.lineStrong, ControlShape)
                .clickable { open = true }
                .padding(horizontal = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                value.ifEmpty { "None" },
                style = CrumbText.body,
                color = if (value.isEmpty()) c.inkMuted else c.ink,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            Icon(Lucide.ChevronDown, null, tint = c.inkMuted, modifier = Modifier.size(16.dp))
        }
        if (open) {
            val density = LocalDensity.current
            Popup(
                alignment = Alignment.TopStart,
                offset = IntOffset(0, with(density) { 46.dp.roundToPx() }),
                onDismissRequest = { open = false },
                properties = PopupProperties(focusable = true),
            ) {
                Column(
                    Modifier
                        .widthIn(min = 200.dp, max = 300.dp)
                        .heightIn(max = 320.dp)
                        .shadow(18.dp, CardShape, ambientColor = c.ink, spotColor = c.ink)
                        .clip(CardShape)
                        .background(c.paper)
                        .border(1.dp, c.line, CardShape)
                        .verticalScroll(rememberScrollState())
                        .padding(vertical = 4.dp),
                ) {
                    CategoryOption("None", "", value, onChange) { open = false }
                    if (legacy != null) CategoryOption("(old) $legacy", legacy, value, onChange) { open = false }
                    categories.forEach { CategoryOption(it, it, value, onChange) { open = false } }
                }
            }
        }
    }
}

@Composable
private fun CategoryOption(label: String, optionValue: String, selected: String, onSelect: (String) -> Unit, onClose: () -> Unit) {
    val c = Crumb.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 44.dp)
            .clickable {
                onSelect(optionValue)
                onClose()
            }
            .padding(horizontal = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, style = CrumbText.body, color = c.ink, modifier = Modifier.weight(1f))
        if (optionValue == selected) Icon(Lucide.Check, null, tint = c.primary, modifier = Modifier.size(18.dp))
    }
}

/** Ingredients or instructions: a header with "+ Section", then one section field per block. */
@Composable
private fun SectionEditor(
    kind: String,
    draft: RecipeDraft,
    onDraft: (RecipeDraft) -> Unit,
    flags: List<Flag>,
    onDismiss: ((Flag) -> Unit)?,
) {
    val c = Crumb.colors
    val ingredients = kind == "ingredients"
    val sections = if (ingredients) draft.ingredients else draft.instructions
    val title = if (ingredients) "Ingredients" else "Instructions"
    val hint = if (ingredients) "One ingredient per line." else "One step per line."
    val namePlaceholder = if (ingredients) "Section name, e.g. For the sauce" else "Section name, e.g. Make the icing"
    val textPlaceholder = if (ingredients) "2 cups flour\n1 tsp salt" else "Preheat the oven to 180°C.\nMix the dry ingredients."

    fun setSections(list: List<DraftSection>) {
        onDraft(if (ingredients) draft.copy(ingredients = list) else draft.copy(instructions = list))
    }

    Column {
        Row(Modifier.fillMaxWidth().padding(bottom = 14.dp), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(title, style = CrumbText.sectionTitle, color = c.ink)
                Text(hint, style = CrumbText.hint, color = c.inkMuted)
            }
            Btn("Section", { setSections(sections + DraftSection()) }, style = BtnStyle.Soft, icon = Lucide.Plus)
        }
        Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
            sections.forEachIndexed { si, section ->
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (sections.size > 1 || section.name.isNotEmpty()) {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            CrumbInput(
                                section.name,
                                { name ->
                                    setSections(sections.toMutableList().also { it[si] = section.copy(name = name) })
                                },
                                modifier = Modifier.weight(1f),
                                placeholder = namePlaceholder,
                                textStyle = CrumbText.body.copy(fontWeight = FontWeight.Bold),
                            )
                            Btn(null, {
                                setSections(sections.filterIndexed { i, _ -> i != si })
                            }, style = BtnStyle.Ghost, icon = Lucide.Trash2, contentDescription = "Remove section")
                        }
                    }
                    CrumbInput(
                        section.text,
                        { text ->
                            setSections(sections.toMutableList().also { it[si] = section.copy(text = text) })
                        },
                        minLines = if (ingredients) 6 else 8,
                        placeholder = textPlaceholder,
                    )
                    val here = section.text.split("\n").map { it.trim() }
                    flags
                        .filter { it.field == kind && it.state == "review" && it.itemText?.let { text -> text in here } == true }
                        .forEach { flag ->
                            FlagHint(
                                flag = flag,
                                fix = fixFor(kind, si, flag, draft),
                                onApplyFix = { fix -> onDraft(fix.apply()) },
                                onDismiss = onDismiss,
                            )
                        }
                }
            }
        }
    }
}

/** One Wee Chef review chip: the quoted line, why it might need a look, a fix and "Keep as is". */
@Composable
private fun FlagHint(flag: Flag, fix: EditorFix?, onApplyFix: (EditorFix) -> Unit, onDismiss: ((Flag) -> Unit)?) {
    val c = Crumb.colors
    Column(
        Modifier
            .fillMaxWidth()
            .clip(ControlShape)
            .background(c.tint)
            .padding(horizontal = 12.dp, vertical = 10.dp),
    ) {
        Row(verticalAlignment = Alignment.Top) {
            Icon(Lucide.ChefHat, null, tint = c.primary, modifier = Modifier.padding(top = 2.dp).size(16.dp))
            HSpace(8.dp)
            Text(
                buildAnnotatedString {
                    withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { append(quote(flag.itemText.orEmpty(), 60)) }
                    append(" ")
                    append(reviewText(flag.kind))
                },
                style = CrumbText.bodySmall,
                color = c.ink,
            )
        }
        Row(
            Modifier.fillMaxWidth().padding(top = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(4.dp, Alignment.End),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (fix != null) {
                Btn(fix.label, { onApplyFix(fix) }, style = BtnStyle.Outline, size = BtnSize.Sm, padding = 14.dp)
            }
            if (onDismiss != null) {
                Btn("Keep as is", { onDismiss(flag) }, style = BtnStyle.Ghost, size = BtnSize.Sm, padding = 14.dp)
            }
        }
    }
}

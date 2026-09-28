package app.crumb.android.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.CloudOff
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import app.crumb.android.ui.theme.Crumb
import coil3.compose.SubcomposeAsyncImage
import com.composables.icons.lucide.CakeSlice
import com.composables.icons.lucide.CookingPot
import com.composables.icons.lucide.Croissant
import com.composables.icons.lucide.Lucide
import com.composables.icons.lucide.Salad
import com.composables.icons.lucide.Sandwich
import com.composables.icons.lucide.Soup

val CardShape = RoundedCornerShape(16.dp)
val ControlShape = RoundedCornerShape(12.dp)

/** A flat paper card with the 1px line border. */
@Composable
fun PaperCard(
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    content: @Composable () -> Unit,
) {
    val colors = Crumb.colors
    if (onClick != null) {
        Surface(
            onClick = onClick,
            modifier = modifier,
            shape = CardShape,
            color = colors.paper,
            border = BorderStroke(1.dp, colors.line),
            content = content,
        )
    } else {
        Surface(
            modifier = modifier,
            shape = CardShape,
            color = colors.paper,
            border = BorderStroke(1.dp, colors.line),
            content = content,
        )
    }
}

/** Butter: only the one main action on a screen. */
@Composable
fun PrimaryButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    icon: ImageVector? = null,
) {
    val colors = Crumb.colors
    Button(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier.heightIn(min = 48.dp),
        shape = ControlShape,
        colors = ButtonDefaults.buttonColors(
            containerColor = colors.butter,
            contentColor = colors.onButter,
            disabledContainerColor = colors.sunk,
            disabledContentColor = colors.inkMuted,
        ),
    ) {
        if (icon != null) {
            Icon(icon, contentDescription = null, modifier = Modifier.size(20.dp))
            Box(Modifier.size(8.dp))
        }
        Text(text, style = MaterialTheme.typography.labelLarge)
    }
}

@Composable
fun SecondaryButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, icon: ImageVector? = null) {
    val colors = Crumb.colors
    OutlinedButton(
        onClick = onClick,
        modifier = modifier.heightIn(min = 48.dp),
        shape = ControlShape,
        border = BorderStroke(1.dp, colors.lineStrong),
        colors = ButtonDefaults.outlinedButtonColors(containerColor = colors.paper, contentColor = colors.ink),
    ) {
        if (icon != null) {
            Icon(icon, contentDescription = null, modifier = Modifier.size(20.dp))
            Box(Modifier.size(8.dp))
        }
        Text(text, style = MaterialTheme.typography.labelLarge)
    }
}

/**
 * A recipe photo, resized by the server. If that fails, the original [image] when it's a
 * web address (as the web app does), else the striped stand-in with the recipe's utensil
 * ([placeholderId] picks it, like Photo.svelte).
 */
@Composable
fun RecipePhoto(
    url: String?,
    image: String?,
    modifier: Modifier = Modifier,
    contentDescription: String? = null,
    placeholderId: Long = 0,
) {
    val original = image?.takeIf { it.startsWith("https://") || it.startsWith("http://") }
    val placeholder = @Composable { PhotoPlaceholder(placeholderId) }
    Box(modifier.clip(ControlShape).background(Crumb.colors.tint)) {
        if (url == null) {
            placeholder()
        } else {
            SubcomposeAsyncImage(
                model = url,
                contentDescription = contentDescription,
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxSize(),
                error = {
                    if (original == null) {
                        placeholder()
                    } else {
                        SubcomposeAsyncImage(
                            model = original,
                            contentDescription = contentDescription,
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                            error = { placeholder() },
                        )
                    }
                },
            )
        }
    }
}

/** The placeholder icons Photo.svelte picks from, by `recipe.id % length`. */
private val PlaceholderIcons =
    listOf(Lucide.CookingPot, Lucide.Soup, Lucide.Salad, Lucide.Croissant, Lucide.CakeSlice, Lucide.Sandwich)

/** The web's striped `.photo-empty` with the recipe's utensil icon. */
@Composable
fun PhotoPlaceholder(id: Long, modifier: Modifier = Modifier) {
    val c = Crumb.colors
    val icon = PlaceholderIcons[(id % PlaceholderIcons.size).toInt()]
    Box(modifier.fillMaxSize().photoEmpty(c.tint, c.paper), contentAlignment = Alignment.Center) {
        Icon(icon, contentDescription = null, tint = c.primary.copy(alpha = 0.7f), modifier = Modifier.size(40.dp))
    }
}

@Composable
fun Loading(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        CircularProgressIndicator(color = Crumb.colors.primary)
    }
}

@Composable
fun Message(
    title: String,
    body: String? = null,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
    action: (@Composable () -> Unit)? = null,
) {
    val colors = Crumb.colors
    Column(
        modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        if (icon != null) {
            Icon(icon, contentDescription = null, tint = colors.primary, modifier = Modifier.size(40.dp))
            Box(Modifier.size(16.dp))
        }
        Text(title, style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center)
        if (body != null) {
            Box(Modifier.size(8.dp))
            Text(body, style = MaterialTheme.typography.bodyMedium, color = colors.inkMuted, textAlign = TextAlign.Center)
        }
        if (action != null) {
            Box(Modifier.size(20.dp))
            action()
        }
    }
}

/** Shown above content that came from the phone because the server didn't answer. */
@Composable
fun OfflineNote(modifier: Modifier = Modifier) {
    val colors = Crumb.colors
    Row(
        modifier
            .fillMaxWidth()
            .clip(ControlShape)
            .background(colors.tint)
            .padding(PaddingValues(horizontal = 14.dp, vertical = 10.dp)),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Outlined.CloudOff, contentDescription = null, tint = colors.primary, modifier = Modifier.size(18.dp))
        Box(Modifier.size(10.dp))
        Text(
            "Offline. Showing what's saved on this phone.",
            style = MaterialTheme.typography.bodySmall,
            color = colors.ink,
        )
    }
}

/** A small tinted pill, for a category or cook time. */
@Composable
fun Pill(text: String, modifier: Modifier = Modifier, icon: ImageVector? = null) {
    val colors = Crumb.colors
    Row(
        modifier
            .clip(RoundedCornerShape(50))
            .background(colors.tint)
            .padding(horizontal = 10.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            Icon(icon, contentDescription = null, tint = colors.primary, modifier = Modifier.size(14.dp))
            Box(Modifier.size(4.dp))
        }
        Text(text, style = MaterialTheme.typography.labelMedium, color = colors.ink)
    }
}

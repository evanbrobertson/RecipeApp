package app.crumb.android.ui.add

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import app.crumb.android.data.CrumbApi
import app.crumb.android.data.PopularLink
import app.crumb.android.data.popularSubtitle
import app.crumb.android.ui.LocalNav
import app.crumb.android.ui.components.CrumbText
import app.crumb.android.ui.components.ListCard
import app.crumb.android.ui.components.ListRow
import app.crumb.android.ui.components.SectionHeader
import app.crumb.android.ui.crumbViewModel
import app.crumb.android.ui.theme.Crumb
import com.composables.icons.lucide.Flame
import com.composables.icons.lucide.Lucide
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * Popular's links, a nice-to-have: any failure, an empty answer, or a server without Popular
 * (password mode, `POPULAR=off`) leaves the Add page as it was.
 */
class PopularViewModel(private val api: CrumbApi) : ViewModel() {
    private val _links = MutableStateFlow<List<PopularLink>>(emptyList())
    val links: StateFlow<List<PopularLink>> = _links.asStateFlow()

    init {
        viewModelScope.launch {
            try {
                _links.value = api.popular()
            } catch (e: CancellationException) {
                throw e
            } catch (_: Exception) {
            }
        }
    }
}

/**
 * "Popular in Crumb" (web/src/islands/PopularLinks.svelte): recipe links several other
 * households saved that this one hasn't. Each opens its preview. Draws nothing when there are none.
 */
@Composable
fun PopularLinks() {
    val c = Crumb.colors
    val nav = LocalNav.current
    val vm = crumbViewModel(key = "popular") { PopularViewModel(it.api) }
    val links by vm.links.collectAsStateWithLifecycle()
    if (links.isEmpty()) return
    Column {
        SectionHeader("Popular in Crumb", Modifier.padding(bottom = 4.dp))
        Text("Saved by several other households.", style = CrumbText.bodySmall, color = c.inkMuted, modifier = Modifier.padding(bottom = 14.dp))
        ListCard(
            rows = links.map { link ->
                {
                    ListRow(
                        link.title,
                        subtitle = popularSubtitle(link),
                        icon = Lucide.Flame,
                        onClick = { nav.preview(link.url) },
                    )
                }
            },
        )
    }
}

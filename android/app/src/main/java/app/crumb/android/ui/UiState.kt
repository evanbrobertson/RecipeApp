package app.crumb.android.ui

import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.crumb.android.AppContainer
import app.crumb.android.CrumbApplication
import app.crumb.android.data.ApiException
import app.crumb.android.data.Loaded
import app.crumb.android.data.OfflineException
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

sealed interface UiState<out T> {
    data object Loading : UiState<Nothing>
    data class Ready<T>(val value: T, val offline: Boolean = false) : UiState<T>
    data class Failed(val message: String, val signedOut: Boolean = false) : UiState<Nothing>
}

/** What to tell someone when a request fails, in kitchen words. */
fun Throwable.friendlyMessage(): String = when (this) {
    is ApiException -> message
    is OfflineException -> "Can't reach your Crumb server. Check your connection and try again."
    is IllegalStateException, is IllegalArgumentException -> message ?: "Something went wrong. Try again."
    else -> "Something went wrong. Try again."
}

/** A screen that loads one thing and can reload it. */
abstract class LoadingViewModel<T> : ViewModel() {
    private val _state = MutableStateFlow<UiState<T>>(UiState.Loading)
    val state: StateFlow<UiState<T>> = _state.asStateFlow()
    private var job: Job? = null

    protected abstract suspend fun fetch(): Loaded<T>

    fun load(showSpinner: Boolean = state.value !is UiState.Ready) {
        job?.cancel()
        if (showSpinner) _state.value = UiState.Loading
        job = viewModelScope.launch {
            _state.value = try {
                val loaded = fetch()
                UiState.Ready(loaded.value, loaded.offline)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                UiState.Failed(e.friendlyMessage(), signedOut = (e as? ApiException)?.isSignedOut == true)
            }
        }
    }
}

val AppContainerProvider: AppContainer
    @Composable get() = (LocalContext.current.applicationContext as CrumbApplication).container

/** `viewModel()` with access to the [AppContainer]. */
@Composable
inline fun <reified VM : ViewModel> crumbViewModel(key: String? = null, crossinline create: (AppContainer) -> VM): VM {
    val container = AppContainerProvider
    return viewModel(key = key, factory = viewModelFactory { initializer { create(container) } })
}

package app.crumb.android.ui.account

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import app.crumb.android.data.Download
import app.crumb.android.ui.components.ToastTone
import app.crumb.android.ui.components.Toaster

/**
 * Saves a downloaded file where the person chooses (the Storage Access Framework's "save
 * as", so no storage permission). Returns the function to call with the download; [saved]
 * is the toast title once it's written.
 */
@Composable
fun rememberFileSaver(saved: String): (Download) -> Unit {
    val context = LocalContext.current
    var pending by remember { mutableStateOf<Download?>(null) }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")) { uri ->
        val download = pending
        pending = null
        if (uri != null && download != null) {
            runCatching { context.contentResolver.openOutputStream(uri)?.use { it.write(download.bytes) } }
                .onSuccess { Toaster.show(saved, tone = ToastTone.Success) }
                .onFailure { Toaster.show("Couldn't save that", it.message ?: "Try again.", ToastTone.Error) }
        }
    }
    return { download ->
        pending = download
        launcher.launch(download.fileName)
    }
}

package app.crumb.android.ui.theme

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.doublePreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

/** The More page's Theme setting. "Sunrise & sunset" is the default, as on the web. */
enum class ThemeMode(val key: String, val label: String, val detail: String) {
    Light("light", "Light", "Always light"),
    Dark("dark", "Dark", "Always dark"),
    System("system", "System", "Follows your device"),
    Sun("sun", "Sunrise & sunset", "Dark from sunset to sunrise"),
    ;

    companion object {
        fun of(key: String?) = entries.firstOrNull { it.key == key } ?: Sun
    }
}

data class ThemeSettings(val mode: ThemeMode = ThemeMode.Sun, val location: SunLocation? = null)

private val Context.themeData: DataStore<Preferences> by preferencesDataStore(name = "theme")

/** Theme mode and the location saved by "Use my location", on this phone only. */
class ThemeStore(private val context: Context) {
    val settings: Flow<ThemeSettings> = context.themeData.data.map { p ->
        val lat = p[LAT]
        val lng = p[LNG]
        ThemeSettings(ThemeMode.of(p[MODE]), if (lat != null && lng != null) SunLocation(lat, lng) else null)
    }

    suspend fun setMode(mode: ThemeMode) {
        context.themeData.edit { it[MODE] = mode.key }
    }

    suspend fun setLocation(location: SunLocation?) {
        context.themeData.edit {
            if (location == null) {
                it.remove(LAT)
                it.remove(LNG)
            } else {
                it[LAT] = location.lat
                it[LNG] = location.lng
            }
        }
    }

    private companion object {
        val MODE = stringPreferencesKey("mode")
        val LAT = doublePreferencesKey("sun_lat")
        val LNG = doublePreferencesKey("sun_lng")
    }
}

package app.crumb.android.data

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import app.crumb.core.InviteLink
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

data class Session(val server: HttpUrl, val cookie: String?)

private val Context.sessionData: DataStore<Preferences> by preferencesDataStore(name = "session")

/**
 * The server address and session cookie. The password is never stored; the cookie is
 * encrypted with a key that lives in the Android Keystore and never leaves the device.
 */
class SessionStore(private val context: Context) : PendingStore {
    private val state = MutableStateFlow<Session?>(null)
    private val loaded = MutableStateFlow(false)

    val session: StateFlow<Session?> = state.asStateFlow()
    val isLoaded: StateFlow<Boolean> = loaded.asStateFlow()
    val current: Session? get() = state.value

    /** The last server used, kept after signing out so it's prefilled next time. */
    var lastServer: String? = null
        private set

    suspend fun load() {
        val prefs = context.sessionData.data.first()
        lastServer = prefs[SERVER]
        val server = prefs[SERVER]?.toHttpUrlOrNull()
        val cookie = prefs[COOKIE]?.let { runCatching { decrypt(it) }.getOrNull() }
        // An empty cookie is a server without a password: signed in, nothing to send.
        state.value = if (server != null && cookie != null) Session(server, cookie.ifEmpty { null }) else null
        loaded.value = true
    }

    override suspend fun signIn(server: HttpUrl, cookie: String?) {
        context.sessionData.edit {
            it[SERVER] = server.toString()
            it[COOKIE] = encrypt(cookie.orEmpty())
        }
        lastServer = server.toString()
        state.value = Session(server, cookie)
    }

    suspend fun signOut() {
        context.sessionData.edit { it.remove(COOKIE) }
        state.value = null
    }

    // What waits while the browser is up or a signed-out person signs in to join: encrypted like
    // the cookie, since the verifier and an invite's token are secrets, and kept apart from the
    // session so signing out (or in) leaves them alone.

    override suspend fun savePendingSignIn(pending: PendingSignIn?) = keep(PENDING_SIGN_IN, pending?.encode())

    override suspend fun loadPendingSignIn(): PendingSignIn? = recall(PENDING_SIGN_IN)?.let(PendingSignIn::decode)

    override suspend fun savePendingInvite(invite: InviteLink?) = keep(PENDING_INVITE, invite?.let { "${it.server}\n${it.token}" })

    override suspend fun loadPendingInvite(): InviteLink? {
        val (server, token) = recall(PENDING_INVITE)?.split('\n', limit = 2)?.takeIf { it.size == 2 } ?: return null
        return InviteLink(server, token)
    }

    private suspend fun keep(key: Preferences.Key<String>, value: String?) {
        context.sessionData.edit { if (value == null) it.remove(key) else it[key] = encrypt(value) }
    }

    private suspend fun recall(key: Preferences.Key<String>): String? =
        context.sessionData.data.first()[key]?.let { runCatching { decrypt(it) }.getOrNull() }

    private fun key(): SecretKey {
        val store = KeyStore.getInstance(KEYSTORE).apply { load(null) }
        (store.getEntry(ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generator.generateKey()
    }

    private fun encrypt(plain: String): String {
        val cipher = Cipher.getInstance(TRANSFORM).apply { init(Cipher.ENCRYPT_MODE, key()) }
        val sealed = cipher.iv + cipher.doFinal(plain.encodeToByteArray())
        return Base64.encodeToString(sealed, Base64.NO_WRAP)
    }

    private fun decrypt(encoded: String): String {
        val sealed = Base64.decode(encoded, Base64.NO_WRAP)
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, sealed, 0, IV_BYTES))
        return cipher.doFinal(sealed, IV_BYTES, sealed.size - IV_BYTES).decodeToString()
    }

    private companion object {
        val SERVER = stringPreferencesKey("server")
        val COOKIE = stringPreferencesKey("cookie")
        val PENDING_SIGN_IN = stringPreferencesKey("pendingSignIn")
        val PENDING_INVITE = stringPreferencesKey("pendingInvite")
        const val KEYSTORE = "AndroidKeyStore"
        const val ALIAS = "crumb-session"
        const val TRANSFORM = "AES/GCM/NoPadding"
        const val IV_BYTES = 12
    }
}

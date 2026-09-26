package app.crumb.android

import android.app.Application
import app.crumb.android.data.CrumbApi
import app.crumb.android.data.RecipeCache
import app.crumb.android.data.RecipeRepository
import app.crumb.android.data.SessionStore
import coil3.ImageLoader
import coil3.PlatformContext
import coil3.SingletonImageLoader
import coil3.disk.DiskCache
import coil3.disk.directory
import coil3.network.okhttp.OkHttpNetworkFetcherFactory
import coil3.request.crossfade
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient
import java.io.File
import java.util.concurrent.TimeUnit

/** Everything the screens share, built once per process. */
class AppContainer(app: Application) {
    val session = SessionStore(app)
    val cache = RecipeCache(File(app.filesDir, "recipes"))

    val http: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        // Imports scrape the page (sometimes in headless Chromium) before answering
        .readTimeout(90, TimeUnit.SECONDS)
        .build()

    val api = CrumbApi(http) { session.current }
    val recipes = RecipeRepository(api, cache)

    /** Photos come from the signed-in server, so they carry the session cookie too. */
    val photoHttp: OkHttpClient = http.newBuilder()
        .addInterceptor { chain ->
            val request = chain.request()
            val current = session.current
            val sameServer = current?.server?.host == request.url.host && current.server.port == request.url.port
            val cookie = current?.cookie
            chain.proceed(
                if (sameServer && cookie != null) {
                    request.newBuilder().header("Cookie", "${CrumbApi.COOKIE}=$cookie").build()
                } else request,
            )
        }
        .build()

    suspend fun signOut() {
        api.logout()
        cache.clear()
        session.signOut()
    }
}

class CrumbApplication : Application(), SingletonImageLoader.Factory {
    lateinit var container: AppContainer
        private set

    private val scope = CoroutineScope(SupervisorJob())

    override fun onCreate() {
        super.onCreate()
        container = AppContainer(this)
        scope.launch {
            container.session.load()
            container.cache.useServer(container.session.current?.server?.toString())
            container.session.session.collect { container.cache.useServer(it?.server?.toString()) }
        }
    }

    override fun newImageLoader(context: PlatformContext): ImageLoader =
        ImageLoader.Builder(context)
            .components { add(OkHttpNetworkFetcherFactory(callFactory = { container.photoHttp })) }
            .diskCache {
                DiskCache.Builder()
                    .directory(cacheDir.resolve("photos"))
                    .maxSizeBytes(200L * 1024 * 1024)
                    .build()
            }
            .crossfade(true)
            .build()
}

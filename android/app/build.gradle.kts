import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

// Release signing comes from android/keystore.properties locally or CRUMB_KEYSTORE_* in CI.
// Without either, release builds are unsigned (CI still checks they compile and shrink).
val keystoreProps = Properties().apply {
    val file = rootProject.file("keystore.properties")
    if (file.exists()) file.inputStream().use(::load)
}

// crumb-core (Rust) via UniFFI: cargo-ndk builds libcrumb_ffi.so for each ABI, and the
// Kotlin bindings are generated from a host build of the same library. The host library also
// backs the JVM unit tests. `-PcrumbAbis=arm64-v8a` builds one ABI for a quicker local loop.
val repoRoot = rootProject.projectDir.parentFile
val cargoTarget = File(repoRoot, "target")
val rustJniLibs = layout.buildDirectory.dir("rustJniLibs")
val uniffiKotlin = layout.buildDirectory.dir("generated/uniffi/kotlin")
val crumbAbis = (findProperty("crumbAbis") as String? ?: "arm64-v8a,armeabi-v7a,x86_64")
    .split(',').map(String::trim).filter(String::isNotEmpty)
val ndkDir = providers.environmentVariable("ANDROID_NDK_HOME").orElse(
    providers.provider {
        val sdk = System.getenv("ANDROID_HOME") ?: Properties().apply {
            rootProject.file("local.properties").takeIf { it.exists() }?.inputStream()?.use(::load)
        }.getProperty("sdk.dir")
        File(sdk, "ndk").listFiles()?.maxByOrNull { it.name }?.path ?: ""
    },
)

val rustSources = fileTree(File(repoRoot, "crates")) {
    include("crumb-core/src/**", "crumb-ffi/src/**", "*/Cargo.toml", "crumb-ffi/uniffi.toml")
}

val cargoHost = tasks.register<Exec>("cargoBuildCoreHost") {
    group = "rust"
    // Dev profile: the release profile strips symbols, and with them UniFFI's metadata
    description = "Builds crumb-ffi for this machine (bindings + JVM tests)."
    workingDir = repoRoot
    inputs.files(rustSources)
    outputs.file(File(cargoTarget, "debug/" + System.mapLibraryName("crumb_ffi")))
    commandLine("cargo", "build", "-p", "crumb-ffi", "--lib")
}

val uniffiBindings = tasks.register<Exec>("generateCoreBindings") {
    group = "rust"
    description = "Generates the Kotlin bindings for crumb-core."
    dependsOn(cargoHost)
    workingDir = repoRoot
    val out = uniffiKotlin.get().asFile
    inputs.files(rustSources)
    outputs.dir(out)
    doFirst { out.deleteRecursively() }
    commandLine(
        "cargo", "run", "-q", "-p", "crumb-ffi", "--features", "bindgen",
        "--bin", "uniffi-bindgen", "--", "generate",
        "--library", File(cargoTarget, "debug/" + System.mapLibraryName("crumb_ffi")).path,
        "--language", "kotlin", "--no-format", "--out-dir", out.path,
    )
}

val cargoAndroid = tasks.register<Exec>("cargoBuildCoreAndroid") {
    group = "rust"
    description = "Cross-compiles crumb-ffi for Android with cargo-ndk."
    workingDir = repoRoot
    val out = rustJniLibs.get().asFile
    inputs.files(rustSources)
    inputs.property("abis", crumbAbis)
    outputs.dir(out)
    environment("ANDROID_NDK_HOME", ndkDir.get())
    commandLine(
        listOf("cargo", "ndk") + crumbAbis.flatMap { listOf("-t", it) } +
            listOf("--platform", "26", "-o", out.path, "build", "--release", "-p", "crumb-ffi", "--lib"),
    )
}

/**
 * The release version as an always-increasing versionCode, so a newer APK installs over an
 * older one: "3.1.2" → 30010029. A master build ("3.1.2-main.40") gets the same number
 * ending in 0, so the promoted release of a version replaces its dev builds.
 */
fun versionCodeFor(version: String): Int {
    val (major, minor, patch) = Regex("""^(\d+)\.(\d+)\.(\d+)""").find(version)
        ?.destructured?.toList()?.map(String::toInt) ?: return 1
    val promoted = if ("-main." in version) 0 else 9
    return major * 10_000_000 + minor * 10_000 + patch * 10 + promoted
}

fun signingValue(key: String, env: String): String? =
    keystoreProps.getProperty(key) ?: providers.environmentVariable(env).orNull

android {
    namespace = "app.crumb.android"
    compileSdk = 37

    defaultConfig {
        applicationId = "app.crumb.android"
        minSdk = 26
        targetSdk = 36
        val version = providers.environmentVariable("CRUMB_VERSION_NAME").orNull ?: "0.1.0"
        versionName = version
        versionCode = providers.environmentVariable("CRUMB_VERSION_CODE").orNull?.toInt()
            ?: versionCodeFor(version)
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        // Only the ABIs crumb-core is built for (JNA alone ships more)
        ndk { abiFilters += crumbAbis }
    }

    signingConfigs {
        val storeFile = signingValue("storeFile", "CRUMB_KEYSTORE_FILE")
        if (storeFile != null) {
            create("release") {
                this.storeFile = rootProject.file(storeFile)
                storePassword = signingValue("storePassword", "CRUMB_KEYSTORE_PASSWORD")
                keyAlias = signingValue("keyAlias", "CRUMB_KEY_ALIAS")
                keyPassword = signingValue("keyPassword", "CRUMB_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        debug {
            applicationIdSuffix = ".debug"
            versionNameSuffix = "-debug"
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    testOptions {
        unitTests.isReturnDefaultValues = true
        unitTests.all {
            it.dependsOn(cargoHost)
            it.systemProperty("jna.library.path", File(cargoTarget, "debug").path)
        }
    }

    sourceSets {
        getByName("main") {
            jniLibs.directories.add(rustJniLibs.get().asFile.path)
            kotlin.directories.add(uniffiKotlin.get().asFile.path)
        }
    }

    lint {
        warningsAsErrors = false
        abortOnError = true
        checkDependencies = true
    }
}

kotlin {
    jvmToolchain(17)
}

tasks.named("preBuild") { dependsOn(uniffiBindings, cargoAndroid) }

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.navigation.compose)
    implementation(libs.androidx.datastore.preferences)

    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.compose.material3)
    implementation(libs.compose.material.icons)

    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.okhttp)
    implementation(libs.coil.compose)
    implementation(libs.coil.network.okhttp)
    implementation("${libs.jna.get()}@aar")

    debugImplementation(libs.compose.ui.tooling)
    debugImplementation(libs.compose.ui.test.manifest)

    testImplementation(libs.junit)
    testImplementation(libs.jna)
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.okhttp.mockwebserver)
}

import org.gradle.api.tasks.PathSensitivity
import java.io.File
import java.util.Properties
import com.hao.bayin.kotlin.BuildTask

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

// Load keystore properties
val keystoreProperties = Properties().apply {
    val keystorePropertiesFile = rootProject.file("keystore.properties")
    if (keystorePropertiesFile.exists()) {
        keystorePropertiesFile.inputStream().use { load(it) }
    }
}

// 优先读环境变量（CI 用），其次读本地 keystore.properties（本地开发用）。
// 这样仓库里不保存任何密钥材料，签名却始终是同一把（可覆盖升级）。
fun signingValue(envKey: String, propKey: String): String? {
    val env = System.getenv(envKey)
    if (!env.isNullOrBlank()) return env
    return keystoreProperties.getProperty(propKey)?.takeIf { it.isNotBlank() }
}

val androidStoreFile = signingValue("ANDROID_KEYSTORE_PATH", "storeFile")
val androidStorePassword = signingValue("ANDROID_KEYSTORE_PASSWORD", "storePassword")
val androidKeyAlias = signingValue("ANDROID_KEY_ALIAS", "keyAlias")
val androidKeyPassword = signingValue("ANDROID_KEY_PASSWORD", "keyPassword")
val hasSigning = androidStoreFile != null && File(androidStoreFile).exists()

android {
    compileSdk = 36
    namespace = "com.hao.bayin"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "true"
        applicationId = "com.hao.bayin"
        minSdk = 24
        targetSdk = 36
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
    }

    // 固定签名配置：同一把 keystore 出的包才能覆盖安装
    signingConfigs {
        create("release") {
            if (hasSigning) {
                storeFile = File(androidStoreFile!!)
                storePassword = androidStorePassword
                keyAlias = androidKeyAlias
                keyPassword = androidKeyPassword
            }
        }
    }

    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            isMinifyEnabled = true
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
            // Apply signing configuration
            if (hasSigning) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        buildConfig = true
    }

    // ABI flavor：名称与 Tauri CLI 的 --target 映射保持一致
    flavorDimensions += "abi"
    productFlavors {
        create("universal") {
            dimension = "abi"
            ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86", "x86_64") }
        }
        create("arm64") {
            dimension = "abi"
            ndk { abiFilters += "arm64-v8a" }
        }
        create("arm") {
            dimension = "abi"
            ndk { abiFilters += "armeabi-v7a" }
        }
        create("x86") {
            dimension = "abi"
            ndk { abiFilters += "x86" }
        }
        create("x86_64") {
            dimension = "abi"
            ndk { abiFilters += "x86_64" }
        }
    }
}

// ── Rust 与 ABI flavor 装配 ─────────────────────────────────────────────
// 原模板通过 buildSrc 的 `rust` 插件完成（依赖 Gradle Plugin Portal 的 kotlin-dsl），
// 这里直接写在模块脚本里，任务名与 flavor 名保持与 Tauri CLI 约定一致：
//   flavor: universal / arm64 / arm / x86 / x86_64
//   task  : rustBuild<Arch><Profile>  → 挂在 merge<Arch><Profile>JniLibFolders 上
val rustRootDirRel = "../../../"
val abiByArch = linkedMapOf(
    "arm64" to "arm64-v8a",
    "arm" to "armeabi-v7a",
    "x86" to "x86",
    "x86_64" to "x86_64"
)
val tripleByArch = linkedMapOf(
    "arm64" to "aarch64-linux-android",
    "arm" to "armv7-linux-androideabi",
    "x86" to "i686-linux-android",
    "x86_64" to "x86_64-linux-android"
)

afterEvaluate {
    for (profile in listOf("debug", "release")) {
        val cap = profile.replaceFirstChar { it.uppercase() }
        val isRelease = profile == "release"

        // universal：一次编完所有 ABI
        val universalTask = tasks.register("rustBuildUniversal$cap") {
            group = "rust"
            description = "Build Rust library for all ABIs ($profile)"
        }
        tasks.matching { it.name == "mergeUniversal${cap}JniLibFolders" }
            .configureEach { dependsOn(universalTask) }

        for ((arch, triple) in tripleByArch) {
            val archCap = arch.replaceFirstChar { it.uppercase() }
            val perArch = tasks.register<BuildTask>("rustBuild$archCap$cap") {
                group = "rust"
                description = "Build Rust library for $arch ($profile)"
                rootDirRel = rustRootDirRel
                target = triple
                release = isRelease
                // 前端产物变了必须重新嵌入二进制，因此把它作为任务输入；
                // 同时禁用 UP-TO-DATE，交给 cargo 自己做增量判断。
                // 相对路径：app → android → gen → src-tauri → 仓库根
                inputs.dir(file("../../../../src-ui-gen")).withPathSensitivity(PathSensitivity.RELATIVE)
                inputs.file(file("../../../Cargo.toml")).withPathSensitivity(PathSensitivity.RELATIVE)
                outputs.upToDateWhen { false }
            }
            universalTask.configure { dependsOn(perArch) }
            tasks.matching { it.name == "merge${archCap}${cap}JniLibFolders" }
                .configureEach { dependsOn(perArch) }
        }
    }
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("androidx.core:core-splashscreen:1.0.1")
    implementation("com.google.android.material:material:1.12.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")
// buildSrc：只提供一个 BuildTask（直接调 cargo），
// flavor 与任务装配放在 app/build.gradle.kts 里用 Kotlin DSL 写，
// 避免依赖 Gradle Plugin Portal 上的 kotlin-dsl 插件。
plugins {
    java
}

java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

repositories {
    google()
    mavenCentral()
}

dependencies {
    compileOnly(gradleApi())
}
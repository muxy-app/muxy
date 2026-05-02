plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.android.library) apply false
    alias(libs.plugins.kotlin.android) apply false
    alias(libs.plugins.kotlin.jvm) apply false
    alias(libs.plugins.kotlin.serialization) apply false
    alias(libs.plugins.kotlin.compose) apply false
    alias(libs.plugins.detekt) apply false
    alias(libs.plugins.ktlint) apply false
}

subprojects {
    apply(plugin = "io.gitlab.arturbosch.detekt")
    apply(plugin = "org.jlleitschuh.gradle.ktlint")

    extensions.configure(io.gitlab.arturbosch.detekt.extensions.DetektExtension::class.java) {
        buildUponDefaultConfig = true
        allRules = false
        config.setFrom(rootProject.files("config/detekt/detekt.yml"))
        ignoreFailures = false
    }
    extensions.configure(org.jlleitschuh.gradle.ktlint.KtlintExtension::class.java) {
        android.set(true)
        ignoreFailures.set(false)
        filter {
            exclude { entry -> entry.file.path.contains("/vendor/") }
            exclude { entry -> entry.file.path.contains("/generated/") }
            exclude { entry -> entry.file.path.contains("/build/") }
        }
    }
}

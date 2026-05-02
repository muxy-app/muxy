import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "com.muxy.terminal"
    compileSdk = 35

    defaultConfig {
        minSdk = 31
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
    }

    sourceSets {
        named("main") {
            java.srcDirs(
                "src/main/java",
                "vendor/terminal-emulator/src/main/java",
                "vendor/terminal-view/src/main/java",
            )
            res.srcDirs(
                "src/main/res",
                "vendor/terminal-view/src/main/res",
            )
        }
        named("test") {
            java.srcDirs("src/test/kotlin")
        }
    }

    lint {
        disable +=
            setOf(
                "DefaultLocale",
                "ObsoleteSdkInt",
                "WrongConstant",
                "ClickableViewAccessibility",
                "RtlHardcoded",
                "InflateParams",
                "UseCompatLoadingForDrawables",
                "Recycle",
                "UseCompatTextViewDrawableXml",
                "PrivateApi",
            )
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

dependencies {
    api(project(":protocol"))
    api(project(":net"))

    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.androidx.annotation)

    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.foundation)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.material.icons.extended)

    testImplementation(libs.junit)

    testImplementation(libs.kotlinx.coroutines.test)
}

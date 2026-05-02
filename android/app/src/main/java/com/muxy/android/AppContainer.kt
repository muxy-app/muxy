package com.muxy.android

import android.app.Application
import android.content.pm.PackageInfo
import android.os.Build
import androidx.compose.runtime.compositionLocalOf
import com.muxy.android.connect.UserPreferences
import com.muxy.android.settings.TerminalPreferences
import com.muxy.net.DeviceCredentialsStore
import com.muxy.net.LastSessionStore
import com.muxy.net.MuxyClient
import com.muxy.net.MuxyLifecycleBinder
import com.muxy.net.SavedDevicesStore

class AppContainer(private val application: Application) {
    val deviceCredentialsStore: DeviceCredentialsStore = DeviceCredentialsStore.create(application)
    val savedDevicesStore: SavedDevicesStore = SavedDevicesStore.create(application)
    val userPreferences: UserPreferences = UserPreferences.create(application)
    val terminalPreferences: TerminalPreferences = TerminalPreferences.create(application)
    val lastSessionStore: LastSessionStore = LastSessionStore.create(application)
    val muxyClient: MuxyClient = MuxyClient(credentialsProvider = deviceCredentialsStore)
    val lifecycleBinder: MuxyLifecycleBinder =
        MuxyLifecycleBinder(
            client = muxyClient,
            connectivityManager = MuxyLifecycleBinder.systemConnectivityManager(application),
        )

    fun appVersionName(): String = packageInfo()?.versionName ?: "-"

    fun appVersionCode(): Long =
        packageInfo()?.let { info ->
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) info.longVersionCode else info.versionCode.toLong()
        } ?: 0L

    private fun packageInfo(): PackageInfo? =
        runCatching {
            application.packageManager.getPackageInfo(application.packageName, 0)
        }.getOrNull()
}

val LocalAppContainer =
    compositionLocalOf<AppContainer> {
        error("AppContainer not provided. Wrap content in CompositionLocalProvider(LocalAppContainer provides ...).")
    }

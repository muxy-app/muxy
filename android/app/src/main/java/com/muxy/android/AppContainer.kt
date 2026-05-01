package com.muxy.android

import android.app.Application
import androidx.compose.runtime.compositionLocalOf
import com.muxy.android.connect.UserPreferences
import com.muxy.net.DeviceCredentialsStore
import com.muxy.net.MuxyClient
import com.muxy.net.SavedDevicesStore

class AppContainer(application: Application) {
    val deviceCredentialsStore: DeviceCredentialsStore = DeviceCredentialsStore.create(application)
    val savedDevicesStore: SavedDevicesStore = SavedDevicesStore.create(application)
    val userPreferences: UserPreferences = UserPreferences.create(application)
    val muxyClient: MuxyClient = MuxyClient(credentialsProvider = deviceCredentialsStore)
}

val LocalAppContainer = compositionLocalOf<AppContainer> {
    error("AppContainer not provided. Wrap content in CompositionLocalProvider(LocalAppContainer provides ...).")
}

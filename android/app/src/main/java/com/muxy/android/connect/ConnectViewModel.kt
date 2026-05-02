package com.muxy.android.connect

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.muxy.net.ConnectionTarget
import com.muxy.net.MuxyClient
import com.muxy.net.SavedDevice
import com.muxy.net.SavedDevicesStore
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

class ConnectViewModel(
    private val savedDevicesStore: SavedDevicesStore,
    private val muxyClient: MuxyClient,
    private val userPreferences: UserPreferences,
    private val defaultPort: Int = DEFAULT_PORT,
) : ViewModel() {
    val savedDevices: StateFlow<List<SavedDevice>> =
        savedDevicesStore.flow.stateIn(
            scope = viewModelScope,
            started = SharingStarted.WhileSubscribed(STOP_TIMEOUT_MS),
            initialValue = emptyList(),
        )

    val trustedNetworkNoticeAcknowledged: StateFlow<Boolean> =
        userPreferences.trustedNetworkNoticeAcknowledged.stateIn(
            scope = viewModelScope,
            started = SharingStarted.WhileSubscribed(STOP_TIMEOUT_MS),
            initialValue = true,
        )

    fun connect(
        name: String,
        host: String,
        port: Int,
    ) {
        val device =
            normalizeConnectInput(name = name, host = host, port = port, defaultPort = defaultPort)
                ?: return
        viewModelScope.launch {
            savedDevicesStore.add(device)
            muxyClient.connect(
                ConnectionTarget(host = device.host, port = device.port, deviceName = device.name),
            )
        }
    }

    fun connect(device: SavedDevice) {
        viewModelScope.launch {
            savedDevicesStore.add(device)
            muxyClient.connect(
                ConnectionTarget(host = device.host, port = device.port, deviceName = device.name),
            )
        }
    }

    fun remove(device: SavedDevice) {
        viewModelScope.launch { savedDevicesStore.remove(device) }
    }

    fun acknowledgeTrustedNetworkNotice() {
        viewModelScope.launch { userPreferences.acknowledgeTrustedNetworkNotice() }
    }

    companion object {
        const val DEFAULT_PORT: Int = 4865
        private const val STOP_TIMEOUT_MS = 5_000L

        fun factory(
            savedDevicesStore: SavedDevicesStore,
            muxyClient: MuxyClient,
            userPreferences: UserPreferences,
        ): ViewModelProvider.Factory =
            viewModelFactory {
                initializer {
                    ConnectViewModel(
                        savedDevicesStore = savedDevicesStore,
                        muxyClient = muxyClient,
                        userPreferences = userPreferences,
                    )
                }
            }
    }
}

internal const val MIN_PORT = 1
internal const val MAX_PORT = 65535

internal fun normalizeConnectInput(
    name: String,
    host: String,
    port: Int,
    defaultPort: Int,
): SavedDevice? {
    val trimmedHost = host.trim()
    if (trimmedHost.isEmpty()) return null
    val trimmedName = name.trim().ifEmpty { "Mac" }
    val resolvedPort = if (port in MIN_PORT..MAX_PORT) port else defaultPort
    return SavedDevice(name = trimmedName, host = trimmedHost, port = resolvedPort)
}

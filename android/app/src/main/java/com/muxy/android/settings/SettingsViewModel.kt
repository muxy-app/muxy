package com.muxy.android.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.muxy.net.DeviceCredentialsStore
import com.muxy.net.LastSessionStore
import com.muxy.net.SavedDevicesStore
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

class SettingsViewModel(
    private val terminalPreferences: TerminalPreferences,
    private val credentialsStore: DeviceCredentialsStore,
    private val savedDevicesStore: SavedDevicesStore,
    private val lastSessionStore: LastSessionStore,
) : ViewModel() {
    val fontSize: StateFlow<Int> =
        terminalPreferences.fontSize.stateIn(
            scope = viewModelScope,
            started = SharingStarted.WhileSubscribed(STOP_TIMEOUT_MS),
            initialValue = TerminalPreferences.DEFAULT_FONT_SIZE,
        )
    val useNerdFont: StateFlow<Boolean> =
        terminalPreferences.useNerdFont.stateIn(
            scope = viewModelScope,
            started = SharingStarted.WhileSubscribed(STOP_TIMEOUT_MS),
            initialValue = false,
        )

    fun setFontSize(size: Int) {
        viewModelScope.launch { terminalPreferences.setFontSize(size) }
    }

    fun setUseNerdFont(enabled: Boolean) {
        viewModelScope.launch { terminalPreferences.setUseNerdFont(enabled) }
    }

    suspend fun forgetDevice() {
        credentialsStore.forget()
        savedDevicesStore.clear()
        lastSessionStore.clear()
    }

    companion object {
        private const val STOP_TIMEOUT_MS = 5_000L

        fun factory(
            terminalPreferences: TerminalPreferences,
            credentialsStore: DeviceCredentialsStore,
            savedDevicesStore: SavedDevicesStore,
            lastSessionStore: LastSessionStore,
        ): ViewModelProvider.Factory =
            viewModelFactory {
                initializer {
                    SettingsViewModel(
                        terminalPreferences = terminalPreferences,
                        credentialsStore = credentialsStore,
                        savedDevicesStore = savedDevicesStore,
                        lastSessionStore = lastSessionStore,
                    )
                }
            }
    }
}

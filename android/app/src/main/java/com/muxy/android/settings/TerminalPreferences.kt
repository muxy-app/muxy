package com.muxy.android.settings

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

private val Context.terminalPreferencesDataStore: DataStore<Preferences> by preferencesDataStore(
    name = "muxy_terminal_preferences",
)

class TerminalPreferences(private val dataStore: DataStore<Preferences>) {
    val fontSize: Flow<Int> =
        dataStore.data.map { prefs ->
            (prefs[KEY_FONT_SIZE] ?: DEFAULT_FONT_SIZE).coerceIn(MIN_FONT_SIZE, MAX_FONT_SIZE)
        }

    val useNerdFont: Flow<Boolean> =
        dataStore.data.map { prefs ->
            prefs[KEY_USE_NERD_FONT] ?: false
        }

    suspend fun setFontSize(size: Int) {
        dataStore.edit { prefs ->
            prefs[KEY_FONT_SIZE] = size.coerceIn(MIN_FONT_SIZE, MAX_FONT_SIZE)
        }
    }

    suspend fun setUseNerdFont(enabled: Boolean) {
        dataStore.edit { prefs -> prefs[KEY_USE_NERD_FONT] = enabled }
    }

    companion object {
        const val DEFAULT_FONT_SIZE: Int = 12
        const val MIN_FONT_SIZE: Int = 8
        const val MAX_FONT_SIZE: Int = 24

        private val KEY_FONT_SIZE = intPreferencesKey("font_size_v1")
        private val KEY_USE_NERD_FONT = booleanPreferencesKey("use_nerd_font_v1")

        fun create(context: Context): TerminalPreferences = TerminalPreferences(context.applicationContext.terminalPreferencesDataStore)
    }
}

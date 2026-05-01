package com.muxy.android.connect

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

private val Context.userPreferencesDataStore: DataStore<Preferences> by preferencesDataStore(
    name = "muxy_user_preferences",
)

class UserPreferences(private val dataStore: DataStore<Preferences>) {
    val trustedNetworkNoticeAcknowledged: Flow<Boolean> = dataStore.data.map { prefs ->
        prefs[KEY_TRUSTED_NETWORK_ACK] ?: false
    }

    suspend fun acknowledgeTrustedNetworkNotice() {
        dataStore.edit { it[KEY_TRUSTED_NETWORK_ACK] = true }
    }

    suspend fun resetTrustedNetworkNotice() {
        dataStore.edit { it.remove(KEY_TRUSTED_NETWORK_ACK) }
    }

    companion object {
        private val KEY_TRUSTED_NETWORK_ACK = booleanPreferencesKey("trusted_network_ack_v1")

        fun create(context: Context): UserPreferences =
            UserPreferences(context.applicationContext.userPreferencesDataStore)
    }
}

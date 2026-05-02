package com.muxy.net

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import java.util.UUID

private val Context.lastSessionDataStore: DataStore<Preferences> by preferencesDataStore(
    name = "muxy_last_session",
)

data class LastSession(
    val deviceName: String,
    val host: String,
    val port: Int,
    val activeProjectID: UUID?,
)

class LastSessionStore(private val dataStore: DataStore<Preferences>) {
    val flow: Flow<LastSession?> =
        dataStore.data.map { prefs ->
            val name = prefs[KEY_NAME] ?: return@map null
            val host = prefs[KEY_HOST] ?: return@map null
            val port = prefs[KEY_PORT] ?: return@map null
            val rawProject = prefs[KEY_PROJECT_ID]
            val projectID = rawProject?.let { runCatching { UUID.fromString(it) }.getOrNull() }
            LastSession(deviceName = name, host = host, port = port, activeProjectID = projectID)
        }

    suspend fun read(): LastSession? = flow.first()

    suspend fun saveTarget(target: ConnectionTarget) {
        dataStore.edit { prefs ->
            prefs[KEY_NAME] = target.deviceName
            prefs[KEY_HOST] = target.host
            prefs[KEY_PORT] = target.port
            prefs.remove(KEY_PROJECT_ID)
        }
    }

    suspend fun saveActiveProject(projectID: UUID?) {
        dataStore.edit { prefs ->
            if (projectID == null) prefs.remove(KEY_PROJECT_ID) else prefs[KEY_PROJECT_ID] = projectID.toString()
        }
    }

    suspend fun clear() {
        dataStore.edit { prefs ->
            prefs.remove(KEY_NAME)
            prefs.remove(KEY_HOST)
            prefs.remove(KEY_PORT)
            prefs.remove(KEY_PROJECT_ID)
        }
    }

    companion object {
        private val KEY_NAME = stringPreferencesKey("device_name")
        private val KEY_HOST = stringPreferencesKey("device_host")
        private val KEY_PORT = intPreferencesKey("device_port")
        private val KEY_PROJECT_ID = stringPreferencesKey("active_project_id")

        fun create(context: Context): LastSessionStore = LastSessionStore(context.applicationContext.lastSessionDataStore)
    }
}

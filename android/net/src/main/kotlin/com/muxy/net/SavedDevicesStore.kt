package com.muxy.net

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.muxy.protocol.codec.MuxyCodec
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.serialization.builtins.ListSerializer

private val Context.savedDevicesDataStore: DataStore<Preferences> by preferencesDataStore(
    name = "muxy_saved_devices",
)

class SavedDevicesStore(private val dataStore: DataStore<Preferences>) {
    val flow: Flow<List<SavedDevice>> =
        dataStore.data.map { prefs ->
            decode(prefs[KEY])
        }

    suspend fun list(): List<SavedDevice> = flow.first()

    suspend fun add(device: SavedDevice) {
        dataStore.edit { prefs ->
            val current = decode(prefs[KEY]).toMutableList()
            current.removeAll { it.host == device.host && it.port == device.port }
            current.add(0, device)
            prefs[KEY] = encode(current)
        }
    }

    suspend fun remove(device: SavedDevice) {
        dataStore.edit { prefs ->
            val current = decode(prefs[KEY]).toMutableList()
            current.removeAll { it.id == device.id }
            prefs[KEY] = encode(current)
        }
    }

    suspend fun clear() {
        dataStore.edit { prefs -> prefs.remove(KEY) }
    }

    private fun decode(raw: String?): List<SavedDevice> {
        if (raw.isNullOrEmpty()) return emptyList()
        return runCatching {
            MuxyCodec.json.decodeFromString(ListSerializer(SavedDevice.serializer()), raw)
        }.getOrDefault(emptyList())
    }

    private fun encode(list: List<SavedDevice>): String = MuxyCodec.json.encodeToString(ListSerializer(SavedDevice.serializer()), list)

    companion object {
        private val KEY = stringPreferencesKey("saved_devices_v1")

        fun create(context: Context): SavedDevicesStore = SavedDevicesStore(context.applicationContext.savedDevicesDataStore)
    }
}

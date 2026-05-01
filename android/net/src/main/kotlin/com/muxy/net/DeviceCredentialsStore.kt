package com.muxy.net

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.muxy.protocol.codec.MuxyCodec
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.Serializable
import java.security.SecureRandom
import java.util.Base64
import java.util.UUID

private val Context.deviceCredentialsDataStore: DataStore<Preferences> by preferencesDataStore(
    name = "muxy_device_credentials",
)

class DeviceCredentialsStore internal constructor(
    private val dataStore: DataStore<Preferences>,
    private val cryptoBox: CryptoBox,
    private val random: SecureRandom = SecureRandom(),
) : DeviceCredentialsProvider {
    private val mutex = Mutex()

    override suspend fun load(): DeviceCredentials = mutex.withLock {
        readDecrypted()?.let { return@withLock it.toDeviceCredentials() }
        val fresh = StoredCredentials(
            deviceID = UUID.randomUUID().toString(),
            token = generateToken(),
        )
        write(fresh)
        fresh.toDeviceCredentials()
    }

    suspend fun forget() = mutex.withLock {
        dataStore.edit { it.remove(KEY) }
        cryptoBox.deleteKey()
    }

    private suspend fun readDecrypted(): StoredCredentials? {
        val raw = dataStore.data.first()[KEY] ?: return null
        val payload = decodePayload(raw) ?: return null
        return runCatching {
            val plaintext = cryptoBox.decrypt(payload)
            MuxyCodec.json.decodeFromString(
                StoredCredentials.serializer(),
                plaintext.toString(Charsets.UTF_8),
            )
        }.getOrNull()
    }

    private suspend fun write(stored: StoredCredentials) {
        val plaintext = MuxyCodec.json
            .encodeToString(StoredCredentials.serializer(), stored)
            .toByteArray(Charsets.UTF_8)
        val payload = cryptoBox.encrypt(plaintext)
        val encoded = encodePayload(payload)
        dataStore.edit { it[KEY] = encoded }
    }

    private fun generateToken(): String {
        val bytes = ByteArray(TOKEN_BYTES)
        random.nextBytes(bytes)
        return Base64.getEncoder().encodeToString(bytes)
    }

    @Serializable
    private data class StoredCredentials(val deviceID: String, val token: String) {
        fun toDeviceCredentials() = DeviceCredentials(
            deviceID = UUID.fromString(deviceID),
            token = token,
        )
    }

    companion object {
        private val KEY = stringPreferencesKey("device_credentials_v1")
        private const val DEFAULT_KEY_ALIAS = "muxy.device_credentials.v1"
        private const val TOKEN_BYTES = 32
        private const val MAX_IV_LENGTH = 255

        fun create(context: Context): DeviceCredentialsStore = DeviceCredentialsStore(
            dataStore = context.applicationContext.deviceCredentialsDataStore,
            cryptoBox = KeystoreCryptoBox(DEFAULT_KEY_ALIAS),
        )

        internal fun encodePayload(payload: EncryptedPayload): String {
            val iv = payload.iv
            val ciphertext = payload.ciphertext
            require(iv.size in 1..MAX_IV_LENGTH) { "iv length out of range: ${iv.size}" }
            val combined = ByteArray(1 + iv.size + ciphertext.size)
            combined[0] = iv.size.toByte()
            System.arraycopy(iv, 0, combined, 1, iv.size)
            System.arraycopy(ciphertext, 0, combined, 1 + iv.size, ciphertext.size)
            return Base64.getEncoder().encodeToString(combined)
        }

        internal fun decodePayload(encoded: String): EncryptedPayload? = runCatching {
            val combined = Base64.getDecoder().decode(encoded)
            if (combined.isEmpty()) return@runCatching null
            val ivLength = combined[0].toInt() and 0xFF
            if (ivLength == 0 || combined.size < 1 + ivLength) return@runCatching null
            EncryptedPayload(
                iv = combined.copyOfRange(1, 1 + ivLength),
                ciphertext = combined.copyOfRange(1 + ivLength, combined.size),
            )
        }.getOrNull()
    }
}

package com.muxy.net

import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.cancel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.security.SecureRandom
import java.util.Base64

class DeviceCredentialsStoreTest {
    @get:Rule val tempFolder = TemporaryFolder()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private lateinit var prefsFile: File
    private lateinit var cryptoBox: FakeCryptoBox
    private lateinit var store: DeviceCredentialsStore

    @Before
    fun setUp() {
        prefsFile = File(tempFolder.newFolder(), "credentials.preferences_pb")
        cryptoBox = FakeCryptoBox()
        store =
            DeviceCredentialsStore(
                dataStore = PreferenceDataStoreFactory.create(scope = scope) { prefsFile },
                cryptoBox = cryptoBox,
                random = SecureRandom(),
            )
    }

    @After
    fun tearDown() {
        scope.cancel()
    }

    @Test
    fun `first load generates persistent credentials`() =
        runBlocking {
            val first = store.load()
            val second = store.load()

            assertEquals(first.deviceID, second.deviceID)
            assertEquals(first.token, second.token)
        }

    @Test
    fun `token is base64 of 32 random bytes`() =
        runBlocking {
            val credentials = store.load()
            val decoded = Base64.getDecoder().decode(credentials.token)
            assertEquals(32, decoded.size)
        }

    @Test
    fun `forget wipes persisted credentials and keystore key`() =
        runBlocking {
            val first = store.load()
            store.forget()

            assertNull(cryptoBox.exportKey())
            val second = store.load()
            assertNotEquals(first.deviceID, second.deviceID)
            assertNotEquals(first.token, second.token)
            assertNotNull(cryptoBox.exportKey())
        }

    @Test
    fun `decrypt failure regenerates fresh credentials`() =
        runBlocking {
            val first = store.load()

            cryptoBox.tamperNextDecrypt = true
            val second = store.load()

            assertNotEquals(first.deviceID, second.deviceID)
            assertNotEquals(first.token, second.token)
        }

    @Test
    fun `payload encoding round-trips iv and ciphertext`() {
        val payload =
            EncryptedPayload(
                iv = ByteArray(12).also { SecureRandom().nextBytes(it) },
                ciphertext = ByteArray(64).also { SecureRandom().nextBytes(it) },
            )
        val encoded = DeviceCredentialsStore.encodePayload(payload)
        val decoded = DeviceCredentialsStore.decodePayload(encoded)

        assertNotNull(decoded)
        assertArrayEquals(payload.iv, decoded!!.iv)
        assertArrayEquals(payload.ciphertext, decoded.ciphertext)
    }

    @Test
    fun `payload decoding rejects malformed input`() {
        assertNull(DeviceCredentialsStore.decodePayload("not-base64-!!!"))
        assertNull(DeviceCredentialsStore.decodePayload(""))
        val emptyIv = Base64.getEncoder().encodeToString(byteArrayOf(0, 1, 2, 3))
        assertNull(DeviceCredentialsStore.decodePayload(emptyIv))
        val ivOverflow = Base64.getEncoder().encodeToString(byteArrayOf(120, 1, 2))
        assertNull(DeviceCredentialsStore.decodePayload(ivOverflow))
    }

    @Test
    fun `concurrent loads return identical credentials`() =
        runBlocking {
            val results =
                coroutineScope {
                    (1..8).map { async { store.load() } }.map { it.await() }
                }
            val first = results.first()
            assertTrue(results.all { it.deviceID == first.deviceID && it.token == first.token })
        }
}

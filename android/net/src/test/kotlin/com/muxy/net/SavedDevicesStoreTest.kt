package com.muxy.net

import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class SavedDevicesStoreTest {
    @get:Rule val tempFolder = TemporaryFolder()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private lateinit var store: SavedDevicesStore
    private lateinit var prefsFile: File

    @Before
    fun setUp() {
        prefsFile = File(tempFolder.newFolder(), "saved.preferences_pb")
        val dataStore = PreferenceDataStoreFactory.create(scope = scope) { prefsFile }
        store = SavedDevicesStore(dataStore)
    }

    @After
    fun tearDown() {
        scope.cancel()
    }

    @Test
    fun `add prepends and dedupes by host port`() = runBlocking {
        store.add(SavedDevice("Mac", "10.0.0.1", 4865))
        store.add(SavedDevice("Mac2", "10.0.0.2", 4865))
        store.add(SavedDevice("Mac-renamed", "10.0.0.1", 4865))

        val list = store.list()
        assertEquals(2, list.size)
        assertEquals("Mac-renamed", list[0].name)
        assertEquals("10.0.0.2", list[1].host)
    }

    @Test
    fun `remove deletes matching device`() = runBlocking {
        val a = SavedDevice("Mac", "10.0.0.1", 4865)
        val b = SavedDevice("Linux", "10.0.0.2", 4865)
        store.add(a)
        store.add(b)
        store.remove(a)

        val list = store.list()
        assertEquals(1, list.size)
        assertEquals("Linux", list[0].name)
    }
}

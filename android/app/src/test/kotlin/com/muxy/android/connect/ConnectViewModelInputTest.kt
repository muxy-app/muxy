package com.muxy.android.connect

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ConnectViewModelInputTest {
    @Test
    fun `empty host returns null`() {
        assertNull(normalizeConnectInput(name = "Mac", host = "  ", port = 4865, defaultPort = 4865))
    }

    @Test
    fun `empty name defaults to Mac`() {
        val device = normalizeConnectInput(name = "  ", host = "10.0.0.1", port = 4865, defaultPort = 4865)
        assertEquals("Mac", device?.name)
    }

    @Test
    fun `whitespace name and host are trimmed`() {
        val device = normalizeConnectInput(
            name = "  Pixel  ",
            host = "  100.64.0.1  ",
            port = 4865,
            defaultPort = 4865,
        )
        assertEquals("Pixel", device?.name)
        assertEquals("100.64.0.1", device?.host)
    }

    @Test
    fun `port out of range falls back to default`() {
        val tooLow = normalizeConnectInput(name = "Mac", host = "10.0.0.1", port = 0, defaultPort = 4865)
        val tooHigh = normalizeConnectInput(name = "Mac", host = "10.0.0.1", port = 70_000, defaultPort = 4865)
        assertEquals(4865, tooLow?.port)
        assertEquals(4865, tooHigh?.port)
    }

    @Test
    fun `valid port is preserved`() {
        val device = normalizeConnectInput(name = "Mac", host = "10.0.0.1", port = 4866, defaultPort = 4865)
        assertEquals(4866, device?.port)
    }
}

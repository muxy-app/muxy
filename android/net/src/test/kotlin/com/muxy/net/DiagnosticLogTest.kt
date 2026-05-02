package com.muxy.net

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant

class DiagnosticLogTest {
    @Test
    fun `ring buffer caps at 120 entries`() {
        val log = DiagnosticLog(capacity = 120)
        repeat(150) { i -> log.append("entry-$i") }
        val snapshot = log.snapshot()
        assertEquals(120, snapshot.size)
        assertTrue(snapshot.first().contains("entry-30"))
        assertTrue(snapshot.last().contains("entry-149"))
    }

    @Test
    fun `lastN returns suffix`() {
        val log = DiagnosticLog(capacity = 120)
        repeat(50) { i -> log.append("e$i") }
        val tail = log.lastN(5)
        assertEquals(5, tail.size)
        assertTrue(tail.last().contains("e49"))
    }

    @Test
    fun `entries include ISO 8601 fractional seconds timestamp`() {
        val log = DiagnosticLog()
        log.append("hi", now = Instant.parse("2026-05-01T12:00:00.250Z"))
        val line = log.snapshot().single()
        assertTrue(line.startsWith("2026-05-01T12:00:00.250"))
    }
}

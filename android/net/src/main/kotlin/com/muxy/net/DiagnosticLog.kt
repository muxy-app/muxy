package com.muxy.net

import java.time.Instant
import java.time.format.DateTimeFormatter
import java.time.format.DateTimeFormatterBuilder
import java.time.temporal.ChronoField
import java.util.Locale

class DiagnosticLog(private val capacity: Int = 120) {
    private val entries = ArrayDeque<String>(capacity)
    private val lock = Any()

    fun append(message: String, now: Instant = Instant.now()) {
        synchronized(lock) {
            entries.addLast("${formatter.format(now)} $message")
            while (entries.size > capacity) {
                entries.removeFirst()
            }
        }
    }

    fun snapshot(): List<String> = synchronized(lock) { entries.toList() }

    fun clear() = synchronized(lock) { entries.clear() }

    fun lastN(n: Int): List<String> = synchronized(lock) {
        if (entries.size <= n) entries.toList() else entries.toList().takeLast(n)
    }

    companion object {
        val formatter: DateTimeFormatter = DateTimeFormatterBuilder()
            .appendPattern("yyyy-MM-dd'T'HH:mm:ss")
            .appendFraction(ChronoField.MILLI_OF_SECOND, 3, 3, true)
            .appendPattern("XXX")
            .toFormatter(Locale.US)
            .withZone(java.time.ZoneOffset.UTC)
    }
}

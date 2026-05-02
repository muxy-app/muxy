package com.muxy.protocol.codec

import com.muxy.protocol.envelope.MuxyMessage
import kotlinx.serialization.json.Json
import kotlinx.serialization.modules.SerializersModule
import kotlinx.serialization.modules.contextual
import java.time.Instant
import java.util.UUID

object MuxyCodec {
    private val module =
        SerializersModule {
            contextual(UUID::class, UuidSerializer)
            contextual(Instant::class, InstantSerializer)
        }

    val json: Json =
        Json {
            explicitNulls = false
            encodeDefaults = false
            ignoreUnknownKeys = true
            serializersModule = module
        }

    fun encode(message: MuxyMessage): String = json.encodeToString(MuxyMessage.serializer(), message)

    fun decode(text: String): MuxyMessage = json.decodeFromString(MuxyMessage.serializer(), text)
}

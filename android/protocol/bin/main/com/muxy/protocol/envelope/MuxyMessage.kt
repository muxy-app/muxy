package com.muxy.protocol.envelope

import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.buildClassSerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonEncoder
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

@Serializable(with = MuxyMessageSerializer::class)
sealed class MuxyMessage {
    data class Request(val value: MuxyRequest) : MuxyMessage()

    data class Response(val value: MuxyResponse) : MuxyMessage()

    data class Event(val value: MuxyEvent) : MuxyMessage()
}

object MuxyMessageSerializer : KSerializer<MuxyMessage> {
    override val descriptor: SerialDescriptor = buildClassSerialDescriptor("MuxyMessage")

    override fun serialize(
        encoder: Encoder,
        value: MuxyMessage,
    ) {
        val output =
            (encoder as? JsonEncoder)
                ?: error("MuxyMessageSerializer only supports JSON")
        val json = output.json
        val element =
            when (value) {
                is MuxyMessage.Request ->
                    buildJsonObject {
                        put("type", "request")
                        put("payload", json.encodeToJsonElement(MuxyRequest.serializer(), value.value))
                    }
                is MuxyMessage.Response ->
                    buildJsonObject {
                        put("type", "response")
                        put("payload", json.encodeToJsonElement(MuxyResponse.serializer(), value.value))
                    }
                is MuxyMessage.Event ->
                    buildJsonObject {
                        put("type", "event")
                        put("payload", json.encodeToJsonElement(MuxyEvent.serializer(), value.value))
                    }
            }
        output.encodeJsonElement(element)
    }

    override fun deserialize(decoder: Decoder): MuxyMessage {
        val input =
            (decoder as? JsonDecoder)
                ?: error("MuxyMessageSerializer only supports JSON")
        val obj = input.decodeJsonElement().jsonObject
        val type = obj.getValue("type").jsonPrimitive.content
        val payload = obj.getValue("payload")
        val json = input.json
        return when (type) {
            "request" -> MuxyMessage.Request(json.decodeFromJsonElement(MuxyRequest.serializer(), payload))
            "response" -> MuxyMessage.Response(json.decodeFromJsonElement(MuxyResponse.serializer(), payload))
            "event" -> MuxyMessage.Event(json.decodeFromJsonElement(MuxyEvent.serializer(), payload))
            else -> error("Unknown MuxyMessage type: $type")
        }
    }
}

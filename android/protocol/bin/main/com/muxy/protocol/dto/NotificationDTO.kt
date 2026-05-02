package com.muxy.protocol.dto

import kotlinx.serialization.Contextual
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
import java.time.Instant
import java.util.UUID

@Serializable
data class NotificationDTO(
    val id: @Contextual UUID,
    val paneID: @Contextual UUID,
    val projectID: @Contextual UUID,
    val worktreeID: @Contextual UUID,
    val areaID: @Contextual UUID,
    val tabID: @Contextual UUID,
    val source: NotificationSourceDTO,
    val title: String,
    val body: String,
    val timestamp: @Contextual Instant,
    val isRead: Boolean,
)

@Serializable(with = NotificationSourceSerializer::class)
sealed class NotificationSourceDTO {
    object Osc : NotificationSourceDTO()

    object Socket : NotificationSourceDTO()

    data class AiProvider(val provider: String) : NotificationSourceDTO()
}

object NotificationSourceSerializer : KSerializer<NotificationSourceDTO> {
    override val descriptor: SerialDescriptor = buildClassSerialDescriptor("NotificationSourceDTO")

    override fun serialize(
        encoder: Encoder,
        value: NotificationSourceDTO,
    ) {
        val output =
            (encoder as? JsonEncoder)
                ?: error("NotificationSourceSerializer only supports JSON")
        val element =
            when (value) {
                is NotificationSourceDTO.Osc ->
                    buildJsonObject {
                        put("osc", buildJsonObject {})
                    }
                is NotificationSourceDTO.Socket ->
                    buildJsonObject {
                        put("socket", buildJsonObject {})
                    }
                is NotificationSourceDTO.AiProvider ->
                    buildJsonObject {
                        put(
                            "aiProvider",
                            buildJsonObject {
                                put("_0", value.provider)
                            },
                        )
                    }
            }
        output.encodeJsonElement(element)
    }

    override fun deserialize(decoder: Decoder): NotificationSourceDTO {
        val input =
            (decoder as? JsonDecoder)
                ?: error("NotificationSourceSerializer only supports JSON")
        val obj = input.decodeJsonElement().jsonObject
        if (obj.containsKey("osc")) return NotificationSourceDTO.Osc
        if (obj.containsKey("socket")) return NotificationSourceDTO.Socket
        if (obj.containsKey("aiProvider")) {
            val inner = obj.getValue("aiProvider").jsonObject
            val provider = inner.getValue("_0").jsonPrimitive.content
            return NotificationSourceDTO.AiProvider(provider)
        }
        error("Unknown NotificationSourceDTO shape: ${obj.keys}")
    }
}

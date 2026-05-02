package com.muxy.protocol.dto

import com.muxy.protocol.codec.UuidSerializer
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
import java.util.UUID

@Serializable(with = PaneOwnerSerializer::class)
sealed class PaneOwnerDTO {
    abstract val displayName: String

    data class Mac(val deviceName: String) : PaneOwnerDTO() {
        override val displayName: String get() = deviceName
    }

    data class Remote(val deviceID: UUID, val deviceName: String) : PaneOwnerDTO() {
        override val displayName: String get() = deviceName
    }
}

object PaneOwnerSerializer : KSerializer<PaneOwnerDTO> {
    override val descriptor: SerialDescriptor = buildClassSerialDescriptor("PaneOwnerDTO")

    override fun serialize(
        encoder: Encoder,
        value: PaneOwnerDTO,
    ) {
        val output =
            (encoder as? JsonEncoder)
                ?: error("PaneOwnerSerializer only supports JSON")
        val element =
            when (value) {
                is PaneOwnerDTO.Mac ->
                    buildJsonObject {
                        put(
                            "mac",
                            buildJsonObject {
                                put("deviceName", value.deviceName)
                            },
                        )
                    }
                is PaneOwnerDTO.Remote ->
                    buildJsonObject {
                        put(
                            "remote",
                            buildJsonObject {
                                put("deviceID", output.json.encodeToJsonElement(UuidSerializer, value.deviceID))
                                put("deviceName", value.deviceName)
                            },
                        )
                    }
            }
        output.encodeJsonElement(element)
    }

    override fun deserialize(decoder: Decoder): PaneOwnerDTO {
        val input =
            (decoder as? JsonDecoder)
                ?: error("PaneOwnerSerializer only supports JSON")
        val obj = input.decodeJsonElement().jsonObject
        if (obj.containsKey("mac")) {
            val inner = obj.getValue("mac").jsonObject
            return PaneOwnerDTO.Mac(deviceName = inner.getValue("deviceName").jsonPrimitive.content)
        }
        if (obj.containsKey("remote")) {
            val inner = obj.getValue("remote").jsonObject
            val deviceID = input.json.decodeFromJsonElement(UuidSerializer, inner.getValue("deviceID"))
            val deviceName = inner.getValue("deviceName").jsonPrimitive.content
            return PaneOwnerDTO.Remote(deviceID = deviceID, deviceName = deviceName)
        }
        error("Unknown PaneOwnerDTO shape: ${obj.keys}")
    }
}

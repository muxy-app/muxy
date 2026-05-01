package com.muxy.protocol.codec

import kotlinx.serialization.KSerializer
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import java.util.Base64

object Base64ByteArraySerializer : KSerializer<ByteArray> {
    override val descriptor: SerialDescriptor =
        PrimitiveSerialDescriptor("Base64ByteArray", PrimitiveKind.STRING)

    private val base64Encoder = Base64.getEncoder()
    private val base64Decoder = Base64.getDecoder()

    override fun serialize(encoder: Encoder, value: ByteArray) {
        encoder.encodeString(base64Encoder.encodeToString(value))
    }

    override fun deserialize(decoder: Decoder): ByteArray {
        return base64Decoder.decode(decoder.decodeString())
    }
}

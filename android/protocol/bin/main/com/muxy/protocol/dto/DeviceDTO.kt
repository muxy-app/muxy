package com.muxy.protocol.dto

import kotlinx.serialization.Contextual
import kotlinx.serialization.Serializable
import java.util.UUID

@Serializable
data class PairingResultDTO(
    val clientID: @Contextual UUID,
    val deviceName: String,
    val themeFg: UInt? = null,
    val themeBg: UInt? = null,
    val themePalette: List<UInt>? = null,
)

@Serializable
data class DeviceInfoDTO(
    val clientID: @Contextual UUID,
    val deviceName: String,
    val themeFg: UInt? = null,
    val themeBg: UInt? = null,
    val themePalette: List<UInt>? = null,
)

@Serializable
data class ProjectLogoDTO(
    val projectID: @Contextual UUID,
    val pngData: String,
)

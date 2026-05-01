package com.muxy.protocol.dto

import kotlinx.serialization.Contextual
import kotlinx.serialization.EncodeDefault
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.Serializable
import java.time.Instant
import java.util.UUID

@OptIn(ExperimentalSerializationApi::class)
@Serializable
data class WorktreeDTO(
    val id: @Contextual UUID,
    val name: String,
    val path: String,
    val branch: String? = null,
    val isPrimary: Boolean,
    @EncodeDefault(EncodeDefault.Mode.ALWAYS)
    val canBeRemoved: Boolean = !isPrimary,
    val createdAt: @Contextual Instant,
)

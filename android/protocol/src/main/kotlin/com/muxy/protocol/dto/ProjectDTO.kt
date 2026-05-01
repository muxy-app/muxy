package com.muxy.protocol.dto

import kotlinx.serialization.Contextual
import kotlinx.serialization.Serializable
import java.time.Instant
import java.util.UUID

@Serializable
data class ProjectDTO(
    val id: @Contextual UUID,
    val name: String,
    val path: String,
    val sortOrder: Int,
    val createdAt: @Contextual Instant,
    val icon: String? = null,
    val logo: String? = null,
    val iconColor: String? = null,
)

package com.muxy.protocol.envelope

import kotlinx.serialization.Serializable

@Serializable
data class MuxyResponse(
    val id: String,
    val result: MuxyResult? = null,
    val error: MuxyError? = null,
)

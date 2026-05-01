package com.muxy.protocol.envelope

import kotlinx.serialization.Serializable

@Serializable
data class MuxyRequest(
    val id: String,
    val method: MuxyMethod,
    val params: MuxyParams? = null,
)

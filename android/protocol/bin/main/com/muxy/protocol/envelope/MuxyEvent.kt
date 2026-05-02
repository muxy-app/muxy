package com.muxy.protocol.envelope

import kotlinx.serialization.Serializable

@Serializable
data class MuxyEvent(
    val event: MuxyEventKind,
    val data: MuxyEventData,
)

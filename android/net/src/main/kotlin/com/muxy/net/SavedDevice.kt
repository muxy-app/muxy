package com.muxy.net

import kotlinx.serialization.Serializable

@Serializable
data class SavedDevice(
    val name: String,
    val host: String,
    val port: Int,
) {
    val id: String get() = "$host:$port"
}

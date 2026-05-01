package com.muxy.protocol.envelope

import kotlinx.serialization.Serializable

@Serializable
data class MuxyError(
    val code: Int,
    val message: String,
) {
    companion object {
        val notFound = MuxyError(code = 404, message = "Not found")
        val invalidParams = MuxyError(code = 400, message = "Invalid parameters")
        val internalError = MuxyError(code = 500, message = "Internal error")
        val unauthorized = MuxyError(code = 401, message = "Authentication required")
        val pairingDenied = MuxyError(code = 403, message = "Pairing denied")
    }
}

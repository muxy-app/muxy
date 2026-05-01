package com.muxy.net

import com.muxy.protocol.envelope.MuxyError

data class ConnectionTarget(
    val host: String,
    val port: Int,
    val deviceName: String,
)

sealed class ConnectionState {
    data object Idle : ConnectionState()
    data class Connecting(val target: ConnectionTarget) : ConnectionState()
    data class Authenticating(val target: ConnectionTarget) : ConnectionState()
    data class AwaitingApproval(val target: ConnectionTarget) : ConnectionState()
    data class Connected(val target: ConnectionTarget) : ConnectionState()
    data class Reconnecting(val target: ConnectionTarget) : ConnectionState()
    data class Failed(val issue: ConnectionIssue, val target: ConnectionTarget?) : ConnectionState()
}

data class ConnectionIssue(
    val message: String,
    val operation: String,
    val timestamp: String,
    val target: ConnectionTarget?,
    val requestMethod: String?,
    val requestID: String?,
    val responseError: MuxyError?,
    val underlyingError: String?,
    val recentLog: List<String>,
)

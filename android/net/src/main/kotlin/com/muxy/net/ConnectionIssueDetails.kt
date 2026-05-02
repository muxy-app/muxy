package com.muxy.net

fun ConnectionIssue.technicalDetails(
    appVersion: String? = null,
    appBuild: String? = null,
    osVersion: String? = null,
    additionalNotes: List<String> = emptyList(),
): String {
    val lines = mutableListOf<String>()
    lines += "Summary: $message"
    lines += "Operation: $operation"
    lines += "Timestamp: $timestamp"
    target?.let {
        lines += "Device: ${it.deviceName}"
        lines += "Target: ${it.host}:${it.port}"
    }
    requestMethod?.let { lines += "Request: $it" }
    requestID?.let { lines += "Request ID: $it" }
    responseError?.let { lines += "Response error: ${it.code} ${it.message}" }
    underlyingError?.let { lines += "Underlying error: $it" }
    appVersion?.let { lines += "App version: $it${appBuild?.let { build -> " ($build)" } ?: ""}" }
    osVersion?.let { lines += "OS: $it" }
    if (additionalNotes.isNotEmpty()) lines += additionalNotes
    if (recentLog.isNotEmpty()) {
        lines += ""
        lines += "Recent connection log:"
        recentLog.takeLast(MAX_LOG_LINES).forEach { lines += "- $it" }
    }
    return lines.joinToString("\n")
}

private const val MAX_LOG_LINES: Int = 25

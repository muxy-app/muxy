package com.muxy.protocol.envelope

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
enum class MuxyEventKind {
    @SerialName("workspaceChanged") WORKSPACE_CHANGED,
    @SerialName("tabChanged") TAB_CHANGED,
    @SerialName("terminalOutput") TERMINAL_OUTPUT,
    @SerialName("terminalSnapshot") TERMINAL_SNAPSHOT,
    @SerialName("notificationReceived") NOTIFICATION_RECEIVED,
    @SerialName("projectsChanged") PROJECTS_CHANGED,
    @SerialName("paneOwnershipChanged") PANE_OWNERSHIP_CHANGED,
    @SerialName("themeChanged") THEME_CHANGED,
}

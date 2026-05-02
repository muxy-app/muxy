package com.muxy.protocol.dto

import com.muxy.protocol.codec.Base64ByteArraySerializer
import com.muxy.protocol.envelope.MuxyEventKind
import kotlinx.serialization.Contextual
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import java.util.UUID

@Serializable
data class SelectProjectParams(val projectID: @Contextual UUID)

@Serializable
data class ListWorktreesParams(val projectID: @Contextual UUID)

@Serializable
data class SelectWorktreeParams(
    val projectID: @Contextual UUID,
    val worktreeID: @Contextual UUID,
)

@Serializable
data class GetWorkspaceParams(val projectID: @Contextual UUID)

@Serializable
data class CreateTabParams(
    val projectID: @Contextual UUID,
    val areaID: @Contextual UUID? = null,
    val kind: TabKindDTO = TabKindDTO.TERMINAL,
)

@Serializable
data class CloseTabParams(
    val projectID: @Contextual UUID,
    val areaID: @Contextual UUID,
    val tabID: @Contextual UUID,
)

@Serializable
data class SelectTabParams(
    val projectID: @Contextual UUID,
    val areaID: @Contextual UUID,
    val tabID: @Contextual UUID,
)

@Serializable
data class SplitAreaParams(
    val projectID: @Contextual UUID,
    val areaID: @Contextual UUID,
    val direction: SplitDirectionDTO,
    val position: SplitPositionDTO,
)

@Serializable
data class CloseAreaParams(
    val projectID: @Contextual UUID,
    val areaID: @Contextual UUID,
)

@Serializable
data class FocusAreaParams(
    val projectID: @Contextual UUID,
    val areaID: @Contextual UUID,
)

@Serializable
data class TerminalInputParams(
    val paneID: @Contextual UUID,
    val bytes:
        @Serializable(with = Base64ByteArraySerializer::class)
        ByteArray,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is TerminalInputParams) return false
        return paneID == other.paneID && bytes.contentEquals(other.bytes)
    }

    override fun hashCode(): Int {
        var result = paneID.hashCode()
        result = 31 * result + bytes.contentHashCode()
        return result
    }
}

@Serializable
data class TerminalResizeParams(
    val paneID: @Contextual UUID,
    val cols: UInt,
    val rows: UInt,
)

@Serializable
data class TerminalScrollParams(
    val paneID: @Contextual UUID,
    val deltaX: Double,
    val deltaY: Double,
    val precise: Boolean,
)

@Serializable
data class GetTerminalContentParams(val paneID: @Contextual UUID)

@Serializable
data class RegisterDeviceParams(val deviceName: String)

@Serializable
data class PairDeviceParams(
    val deviceID: @Contextual UUID,
    val deviceName: String,
    val token: String,
)

@Serializable
data class AuthenticateDeviceParams(
    val deviceID: @Contextual UUID,
    val deviceName: String,
    val token: String,
)

@Serializable
data class TakeOverPaneParams(
    val paneID: @Contextual UUID,
    val cols: UInt,
    val rows: UInt,
)

@Serializable
data class ReleasePaneParams(val paneID: @Contextual UUID)

@Serializable
data class PaneOwnershipEventDTO(
    val paneID: @Contextual UUID,
    val owner: PaneOwnerDTO,
)

@Serializable
data class DeviceThemeEventDTO(
    val fg: UInt,
    val bg: UInt,
    val palette: List<UInt>? = null,
)

@Serializable
data class TabChangeEventDTO(
    val projectID: @Contextual UUID,
    val areaID: @Contextual UUID,
    val tab: TabDTO,
    val changeKind: TabChangeKind,
) {
    @Serializable
    enum class TabChangeKind {
        @SerialName("created")
        CREATED,

        @SerialName("closed")
        CLOSED,

        @SerialName("selected")
        SELECTED,

        @SerialName("titleChanged")
        TITLE_CHANGED,
    }
}

@Serializable
data class GetVCSStatusParams(val projectID: @Contextual UUID)

@Serializable
data class VCSCommitParams(
    val projectID: @Contextual UUID,
    val message: String,
    val stageAll: Boolean = false,
)

@Serializable
data class VCSPushParams(val projectID: @Contextual UUID)

@Serializable
data class VCSPullParams(val projectID: @Contextual UUID)

@Serializable
data class VCSStageFilesParams(
    val projectID: @Contextual UUID,
    val paths: List<String>,
)

@Serializable
data class VCSUnstageFilesParams(
    val projectID: @Contextual UUID,
    val paths: List<String>,
)

@Serializable
data class VCSDiscardFilesParams(
    val projectID: @Contextual UUID,
    val paths: List<String>,
    val untrackedPaths: List<String>,
)

@Serializable
data class VCSListBranchesParams(val projectID: @Contextual UUID)

@Serializable
data class VCSSwitchBranchParams(
    val projectID: @Contextual UUID,
    val branch: String,
)

@Serializable
data class VCSCreateBranchParams(
    val projectID: @Contextual UUID,
    val name: String,
)

@Serializable
data class VCSCreatePRParams(
    val projectID: @Contextual UUID,
    val title: String,
    val body: String,
    val baseBranch: String? = null,
    val draft: Boolean,
)

@Serializable
data class VCSAddWorktreeParams(
    val projectID: @Contextual UUID,
    val name: String,
    val branch: String,
    val createBranch: Boolean,
)

@Serializable
data class VCSRemoveWorktreeParams(
    val projectID: @Contextual UUID,
    val worktreeID: @Contextual UUID,
)

@Serializable
data class GetProjectLogoParams(val projectID: @Contextual UUID)

@Serializable
data class MarkNotificationReadParams(val notificationID: @Contextual UUID)

@Serializable
data class SubscribeParams(val events: List<MuxyEventKind>)

@Serializable
data class UnsubscribeParams(val events: List<MuxyEventKind>)

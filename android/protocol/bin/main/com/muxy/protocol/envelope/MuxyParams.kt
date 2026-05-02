package com.muxy.protocol.envelope

import com.muxy.protocol.dto.AuthenticateDeviceParams
import com.muxy.protocol.dto.CloseAreaParams
import com.muxy.protocol.dto.CloseTabParams
import com.muxy.protocol.dto.CreateTabParams
import com.muxy.protocol.dto.FocusAreaParams
import com.muxy.protocol.dto.GetProjectLogoParams
import com.muxy.protocol.dto.GetTerminalContentParams
import com.muxy.protocol.dto.GetVCSStatusParams
import com.muxy.protocol.dto.GetWorkspaceParams
import com.muxy.protocol.dto.ListWorktreesParams
import com.muxy.protocol.dto.MarkNotificationReadParams
import com.muxy.protocol.dto.PairDeviceParams
import com.muxy.protocol.dto.RegisterDeviceParams
import com.muxy.protocol.dto.ReleasePaneParams
import com.muxy.protocol.dto.SelectProjectParams
import com.muxy.protocol.dto.SelectTabParams
import com.muxy.protocol.dto.SelectWorktreeParams
import com.muxy.protocol.dto.SplitAreaParams
import com.muxy.protocol.dto.SubscribeParams
import com.muxy.protocol.dto.TakeOverPaneParams
import com.muxy.protocol.dto.TerminalInputParams
import com.muxy.protocol.dto.TerminalResizeParams
import com.muxy.protocol.dto.TerminalScrollParams
import com.muxy.protocol.dto.UnsubscribeParams
import com.muxy.protocol.dto.VCSAddWorktreeParams
import com.muxy.protocol.dto.VCSCommitParams
import com.muxy.protocol.dto.VCSCreateBranchParams
import com.muxy.protocol.dto.VCSCreatePRParams
import com.muxy.protocol.dto.VCSDiscardFilesParams
import com.muxy.protocol.dto.VCSListBranchesParams
import com.muxy.protocol.dto.VCSPullParams
import com.muxy.protocol.dto.VCSPushParams
import com.muxy.protocol.dto.VCSRemoveWorktreeParams
import com.muxy.protocol.dto.VCSStageFilesParams
import com.muxy.protocol.dto.VCSSwitchBranchParams
import com.muxy.protocol.dto.VCSUnstageFilesParams
import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.buildClassSerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonEncoder
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

@Serializable(with = MuxyParamsSerializer::class)
sealed class MuxyParams {
    data class SelectProject(val value: SelectProjectParams) : MuxyParams()

    data class ListWorktrees(val value: ListWorktreesParams) : MuxyParams()

    data class SelectWorktree(val value: SelectWorktreeParams) : MuxyParams()

    data class GetWorkspace(val value: GetWorkspaceParams) : MuxyParams()

    data class CreateTab(val value: CreateTabParams) : MuxyParams()

    data class CloseTab(val value: CloseTabParams) : MuxyParams()

    data class SelectTab(val value: SelectTabParams) : MuxyParams()

    data class SplitArea(val value: SplitAreaParams) : MuxyParams()

    data class CloseArea(val value: CloseAreaParams) : MuxyParams()

    data class FocusArea(val value: FocusAreaParams) : MuxyParams()

    data class TerminalInput(val value: TerminalInputParams) : MuxyParams()

    data class TerminalResize(val value: TerminalResizeParams) : MuxyParams()

    data class TerminalScroll(val value: TerminalScrollParams) : MuxyParams()

    data class GetTerminalContent(val value: GetTerminalContentParams) : MuxyParams()

    data class RegisterDevice(val value: RegisterDeviceParams) : MuxyParams()

    data class PairDevice(val value: PairDeviceParams) : MuxyParams()

    data class AuthenticateDevice(val value: AuthenticateDeviceParams) : MuxyParams()

    data class TakeOverPane(val value: TakeOverPaneParams) : MuxyParams()

    data class ReleasePane(val value: ReleasePaneParams) : MuxyParams()

    data class GetVCSStatus(val value: GetVCSStatusParams) : MuxyParams()

    data class VCSCommit(val value: VCSCommitParams) : MuxyParams()

    data class VCSPush(val value: VCSPushParams) : MuxyParams()

    data class VCSPull(val value: VCSPullParams) : MuxyParams()

    data class VCSStageFiles(val value: VCSStageFilesParams) : MuxyParams()

    data class VCSUnstageFiles(val value: VCSUnstageFilesParams) : MuxyParams()

    data class VCSDiscardFiles(val value: VCSDiscardFilesParams) : MuxyParams()

    data class VCSListBranches(val value: VCSListBranchesParams) : MuxyParams()

    data class VCSSwitchBranch(val value: VCSSwitchBranchParams) : MuxyParams()

    data class VCSCreateBranch(val value: VCSCreateBranchParams) : MuxyParams()

    data class VCSCreatePR(val value: VCSCreatePRParams) : MuxyParams()

    data class VCSAddWorktree(val value: VCSAddWorktreeParams) : MuxyParams()

    data class VCSRemoveWorktree(val value: VCSRemoveWorktreeParams) : MuxyParams()

    data class GetProjectLogo(val value: GetProjectLogoParams) : MuxyParams()

    data class MarkNotificationRead(val value: MarkNotificationReadParams) : MuxyParams()

    data class Subscribe(val value: SubscribeParams) : MuxyParams()

    data class Unsubscribe(val value: UnsubscribeParams) : MuxyParams()
}

object MuxyParamsSerializer : KSerializer<MuxyParams> {
    override val descriptor: SerialDescriptor = buildClassSerialDescriptor("MuxyParams")

    override fun serialize(
        encoder: Encoder,
        value: MuxyParams,
    ) {
        val output =
            (encoder as? JsonEncoder)
                ?: error("MuxyParamsSerializer only supports JSON")
        val (typeKey, jsonValue) = serializeBranch(output, value)
        val element =
            buildJsonObject {
                put("type", typeKey)
                put("value", jsonValue)
            }
        output.encodeJsonElement(element)
    }

    override fun deserialize(decoder: Decoder): MuxyParams {
        val input =
            (decoder as? JsonDecoder)
                ?: error("MuxyParamsSerializer only supports JSON")
        val obj = input.decodeJsonElement().jsonObject
        val type = obj.getValue("type").jsonPrimitive.content
        val value = obj.getValue("value")
        return deserializeBranch(input, type, value)
    }

    private fun serializeBranch(
        output: JsonEncoder,
        value: MuxyParams,
    ): Pair<String, JsonElement> {
        val json = output.json
        return when (value) {
            is MuxyParams.SelectProject -> "selectProject" to json.encodeToJsonElement(SelectProjectParams.serializer(), value.value)
            is MuxyParams.ListWorktrees -> "listWorktrees" to json.encodeToJsonElement(ListWorktreesParams.serializer(), value.value)
            is MuxyParams.SelectWorktree -> "selectWorktree" to json.encodeToJsonElement(SelectWorktreeParams.serializer(), value.value)
            is MuxyParams.GetWorkspace -> "getWorkspace" to json.encodeToJsonElement(GetWorkspaceParams.serializer(), value.value)
            is MuxyParams.CreateTab -> "createTab" to json.encodeToJsonElement(CreateTabParams.serializer(), value.value)
            is MuxyParams.CloseTab -> "closeTab" to json.encodeToJsonElement(CloseTabParams.serializer(), value.value)
            is MuxyParams.SelectTab -> "selectTab" to json.encodeToJsonElement(SelectTabParams.serializer(), value.value)
            is MuxyParams.SplitArea -> "splitArea" to json.encodeToJsonElement(SplitAreaParams.serializer(), value.value)
            is MuxyParams.CloseArea -> "closeArea" to json.encodeToJsonElement(CloseAreaParams.serializer(), value.value)
            is MuxyParams.FocusArea -> "focusArea" to json.encodeToJsonElement(FocusAreaParams.serializer(), value.value)
            is MuxyParams.TerminalInput -> "terminalInput" to json.encodeToJsonElement(TerminalInputParams.serializer(), value.value)
            is MuxyParams.TerminalResize -> "terminalResize" to json.encodeToJsonElement(TerminalResizeParams.serializer(), value.value)
            is MuxyParams.TerminalScroll -> "terminalScroll" to json.encodeToJsonElement(TerminalScrollParams.serializer(), value.value)
            is MuxyParams.GetTerminalContent -> "getTerminalContent" to json.encodeToJsonElement(GetTerminalContentParams.serializer(), value.value)
            is MuxyParams.RegisterDevice -> "registerDevice" to json.encodeToJsonElement(RegisterDeviceParams.serializer(), value.value)
            is MuxyParams.PairDevice -> "pairDevice" to json.encodeToJsonElement(PairDeviceParams.serializer(), value.value)
            is MuxyParams.AuthenticateDevice -> "authenticateDevice" to json.encodeToJsonElement(AuthenticateDeviceParams.serializer(), value.value)
            is MuxyParams.TakeOverPane -> "takeOverPane" to json.encodeToJsonElement(TakeOverPaneParams.serializer(), value.value)
            is MuxyParams.ReleasePane -> "releasePane" to json.encodeToJsonElement(ReleasePaneParams.serializer(), value.value)
            is MuxyParams.GetVCSStatus -> "getVCSStatus" to json.encodeToJsonElement(GetVCSStatusParams.serializer(), value.value)
            is MuxyParams.VCSCommit -> "vcsCommit" to json.encodeToJsonElement(VCSCommitParams.serializer(), value.value)
            is MuxyParams.VCSPush -> "vcsPush" to json.encodeToJsonElement(VCSPushParams.serializer(), value.value)
            is MuxyParams.VCSPull -> "vcsPull" to json.encodeToJsonElement(VCSPullParams.serializer(), value.value)
            is MuxyParams.VCSStageFiles -> "vcsStageFiles" to json.encodeToJsonElement(VCSStageFilesParams.serializer(), value.value)
            is MuxyParams.VCSUnstageFiles -> "vcsUnstageFiles" to json.encodeToJsonElement(VCSUnstageFilesParams.serializer(), value.value)
            is MuxyParams.VCSDiscardFiles -> "vcsDiscardFiles" to json.encodeToJsonElement(VCSDiscardFilesParams.serializer(), value.value)
            is MuxyParams.VCSListBranches -> "vcsListBranches" to json.encodeToJsonElement(VCSListBranchesParams.serializer(), value.value)
            is MuxyParams.VCSSwitchBranch -> "vcsSwitchBranch" to json.encodeToJsonElement(VCSSwitchBranchParams.serializer(), value.value)
            is MuxyParams.VCSCreateBranch -> "vcsCreateBranch" to json.encodeToJsonElement(VCSCreateBranchParams.serializer(), value.value)
            is MuxyParams.VCSCreatePR -> "vcsCreatePR" to json.encodeToJsonElement(VCSCreatePRParams.serializer(), value.value)
            is MuxyParams.VCSAddWorktree -> "vcsAddWorktree" to json.encodeToJsonElement(VCSAddWorktreeParams.serializer(), value.value)
            is MuxyParams.VCSRemoveWorktree -> "vcsRemoveWorktree" to json.encodeToJsonElement(VCSRemoveWorktreeParams.serializer(), value.value)
            is MuxyParams.GetProjectLogo -> "getProjectLogo" to json.encodeToJsonElement(GetProjectLogoParams.serializer(), value.value)
            is MuxyParams.MarkNotificationRead -> "markNotificationRead" to json.encodeToJsonElement(MarkNotificationReadParams.serializer(), value.value)
            is MuxyParams.Subscribe -> "subscribe" to json.encodeToJsonElement(SubscribeParams.serializer(), value.value)
            is MuxyParams.Unsubscribe -> "unsubscribe" to json.encodeToJsonElement(UnsubscribeParams.serializer(), value.value)
        }
    }

    private fun deserializeBranch(
        input: JsonDecoder,
        type: String,
        value: JsonElement,
    ): MuxyParams {
        val json = input.json
        return when (type) {
            "selectProject" -> MuxyParams.SelectProject(json.decodeFromJsonElement(SelectProjectParams.serializer(), value))
            "listWorktrees" -> MuxyParams.ListWorktrees(json.decodeFromJsonElement(ListWorktreesParams.serializer(), value))
            "selectWorktree" -> MuxyParams.SelectWorktree(json.decodeFromJsonElement(SelectWorktreeParams.serializer(), value))
            "getWorkspace" -> MuxyParams.GetWorkspace(json.decodeFromJsonElement(GetWorkspaceParams.serializer(), value))
            "createTab" -> MuxyParams.CreateTab(json.decodeFromJsonElement(CreateTabParams.serializer(), value))
            "closeTab" -> MuxyParams.CloseTab(json.decodeFromJsonElement(CloseTabParams.serializer(), value))
            "selectTab" -> MuxyParams.SelectTab(json.decodeFromJsonElement(SelectTabParams.serializer(), value))
            "splitArea" -> MuxyParams.SplitArea(json.decodeFromJsonElement(SplitAreaParams.serializer(), value))
            "closeArea" -> MuxyParams.CloseArea(json.decodeFromJsonElement(CloseAreaParams.serializer(), value))
            "focusArea" -> MuxyParams.FocusArea(json.decodeFromJsonElement(FocusAreaParams.serializer(), value))
            "terminalInput" -> MuxyParams.TerminalInput(json.decodeFromJsonElement(TerminalInputParams.serializer(), value))
            "terminalResize" -> MuxyParams.TerminalResize(json.decodeFromJsonElement(TerminalResizeParams.serializer(), value))
            "terminalScroll" -> MuxyParams.TerminalScroll(json.decodeFromJsonElement(TerminalScrollParams.serializer(), value))
            "getTerminalContent" -> MuxyParams.GetTerminalContent(json.decodeFromJsonElement(GetTerminalContentParams.serializer(), value))
            "registerDevice" -> MuxyParams.RegisterDevice(json.decodeFromJsonElement(RegisterDeviceParams.serializer(), value))
            "pairDevice" -> MuxyParams.PairDevice(json.decodeFromJsonElement(PairDeviceParams.serializer(), value))
            "authenticateDevice" -> MuxyParams.AuthenticateDevice(json.decodeFromJsonElement(AuthenticateDeviceParams.serializer(), value))
            "takeOverPane" -> MuxyParams.TakeOverPane(json.decodeFromJsonElement(TakeOverPaneParams.serializer(), value))
            "releasePane" -> MuxyParams.ReleasePane(json.decodeFromJsonElement(ReleasePaneParams.serializer(), value))
            "getVCSStatus" -> MuxyParams.GetVCSStatus(json.decodeFromJsonElement(GetVCSStatusParams.serializer(), value))
            "vcsCommit" -> MuxyParams.VCSCommit(json.decodeFromJsonElement(VCSCommitParams.serializer(), value))
            "vcsPush" -> MuxyParams.VCSPush(json.decodeFromJsonElement(VCSPushParams.serializer(), value))
            "vcsPull" -> MuxyParams.VCSPull(json.decodeFromJsonElement(VCSPullParams.serializer(), value))
            "vcsStageFiles" -> MuxyParams.VCSStageFiles(json.decodeFromJsonElement(VCSStageFilesParams.serializer(), value))
            "vcsUnstageFiles" -> MuxyParams.VCSUnstageFiles(json.decodeFromJsonElement(VCSUnstageFilesParams.serializer(), value))
            "vcsDiscardFiles" -> MuxyParams.VCSDiscardFiles(json.decodeFromJsonElement(VCSDiscardFilesParams.serializer(), value))
            "vcsListBranches" -> MuxyParams.VCSListBranches(json.decodeFromJsonElement(VCSListBranchesParams.serializer(), value))
            "vcsSwitchBranch" -> MuxyParams.VCSSwitchBranch(json.decodeFromJsonElement(VCSSwitchBranchParams.serializer(), value))
            "vcsCreateBranch" -> MuxyParams.VCSCreateBranch(json.decodeFromJsonElement(VCSCreateBranchParams.serializer(), value))
            "vcsCreatePR" -> MuxyParams.VCSCreatePR(json.decodeFromJsonElement(VCSCreatePRParams.serializer(), value))
            "vcsAddWorktree" -> MuxyParams.VCSAddWorktree(json.decodeFromJsonElement(VCSAddWorktreeParams.serializer(), value))
            "vcsRemoveWorktree" -> MuxyParams.VCSRemoveWorktree(json.decodeFromJsonElement(VCSRemoveWorktreeParams.serializer(), value))
            "getProjectLogo" -> MuxyParams.GetProjectLogo(json.decodeFromJsonElement(GetProjectLogoParams.serializer(), value))
            "markNotificationRead" ->
                MuxyParams.MarkNotificationRead(
                    json.decodeFromJsonElement(MarkNotificationReadParams.serializer(), value),
                )
            "subscribe" -> MuxyParams.Subscribe(json.decodeFromJsonElement(SubscribeParams.serializer(), value))
            "unsubscribe" -> MuxyParams.Unsubscribe(json.decodeFromJsonElement(UnsubscribeParams.serializer(), value))
            else -> error("Unknown MuxyParams type: $type")
        }
    }
}

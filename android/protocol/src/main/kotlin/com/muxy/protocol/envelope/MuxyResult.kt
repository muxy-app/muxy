package com.muxy.protocol.envelope

import com.muxy.protocol.dto.DeviceInfoDTO
import com.muxy.protocol.dto.NotificationDTO
import com.muxy.protocol.dto.PairingResultDTO
import com.muxy.protocol.dto.PaneOwnerDTO
import com.muxy.protocol.dto.ProjectDTO
import com.muxy.protocol.dto.ProjectLogoDTO
import com.muxy.protocol.dto.TabDTO
import com.muxy.protocol.dto.TerminalCellsDTO
import com.muxy.protocol.dto.TerminalContentDTO
import com.muxy.protocol.dto.VCSBranchesDTO
import com.muxy.protocol.dto.VCSCreatePRResultDTO
import com.muxy.protocol.dto.VCSStatusDTO
import com.muxy.protocol.dto.WorkspaceDTO
import com.muxy.protocol.dto.WorktreeDTO
import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
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

@Serializable(with = MuxyResultSerializer::class)
sealed class MuxyResult {
    data class Projects(val value: List<ProjectDTO>) : MuxyResult()

    data class Worktrees(val value: List<WorktreeDTO>) : MuxyResult()

    data class Workspace(val value: WorkspaceDTO) : MuxyResult()

    data class Tab(val value: TabDTO) : MuxyResult()

    data class TerminalContent(val value: TerminalContentDTO) : MuxyResult()

    data class TerminalCells(val value: TerminalCellsDTO) : MuxyResult()

    data class DeviceInfo(val value: DeviceInfoDTO) : MuxyResult()

    data class Pairing(val value: PairingResultDTO) : MuxyResult()

    data class PaneOwner(val value: PaneOwnerDTO) : MuxyResult()

    data class VCSStatus(val value: VCSStatusDTO) : MuxyResult()

    data class VCSBranches(val value: VCSBranchesDTO) : MuxyResult()

    data class VCSPRCreated(val value: VCSCreatePRResultDTO) : MuxyResult()

    data class ProjectLogo(val value: ProjectLogoDTO) : MuxyResult()

    data class Notifications(val value: List<NotificationDTO>) : MuxyResult()

    object Ok : MuxyResult()
}

object MuxyResultSerializer : KSerializer<MuxyResult> {
    override val descriptor: SerialDescriptor = buildClassSerialDescriptor("MuxyResult")

    override fun serialize(
        encoder: Encoder,
        value: MuxyResult,
    ) {
        val output =
            (encoder as? JsonEncoder)
                ?: error("MuxyResultSerializer only supports JSON")
        val json = output.json
        val element =
            when (value) {
                is MuxyResult.Projects ->
                    buildJsonObject {
                        put("type", "projects")
                        put("value", json.encodeToJsonElement(ListSerializer(ProjectDTO.serializer()), value.value))
                    }
                is MuxyResult.Worktrees ->
                    buildJsonObject {
                        put("type", "worktrees")
                        put("value", json.encodeToJsonElement(ListSerializer(WorktreeDTO.serializer()), value.value))
                    }
                is MuxyResult.Workspace ->
                    buildJsonObject {
                        put("type", "workspace")
                        put("value", json.encodeToJsonElement(WorkspaceDTO.serializer(), value.value))
                    }
                is MuxyResult.Tab ->
                    buildJsonObject {
                        put("type", "tab")
                        put("value", json.encodeToJsonElement(TabDTO.serializer(), value.value))
                    }
                is MuxyResult.TerminalContent ->
                    buildJsonObject {
                        put("type", "terminalContent")
                        put("value", json.encodeToJsonElement(TerminalContentDTO.serializer(), value.value))
                    }
                is MuxyResult.TerminalCells ->
                    buildJsonObject {
                        put("type", "terminalCells")
                        put("value", json.encodeToJsonElement(TerminalCellsDTO.serializer(), value.value))
                    }
                is MuxyResult.DeviceInfo ->
                    buildJsonObject {
                        put("type", "deviceInfo")
                        put("value", json.encodeToJsonElement(DeviceInfoDTO.serializer(), value.value))
                    }
                is MuxyResult.Pairing ->
                    buildJsonObject {
                        put("type", "pairing")
                        put("value", json.encodeToJsonElement(PairingResultDTO.serializer(), value.value))
                    }
                is MuxyResult.PaneOwner ->
                    buildJsonObject {
                        put("type", "paneOwner")
                        put("value", json.encodeToJsonElement(PaneOwnerDTO.serializer(), value.value))
                    }
                is MuxyResult.VCSStatus ->
                    buildJsonObject {
                        put("type", "vcsStatus")
                        put("value", json.encodeToJsonElement(VCSStatusDTO.serializer(), value.value))
                    }
                is MuxyResult.VCSBranches ->
                    buildJsonObject {
                        put("type", "vcsBranches")
                        put("value", json.encodeToJsonElement(VCSBranchesDTO.serializer(), value.value))
                    }
                is MuxyResult.VCSPRCreated ->
                    buildJsonObject {
                        put("type", "vcsPRCreated")
                        put("value", json.encodeToJsonElement(VCSCreatePRResultDTO.serializer(), value.value))
                    }
                is MuxyResult.ProjectLogo ->
                    buildJsonObject {
                        put("type", "projectLogo")
                        put("value", json.encodeToJsonElement(ProjectLogoDTO.serializer(), value.value))
                    }
                is MuxyResult.Notifications ->
                    buildJsonObject {
                        put("type", "notifications")
                        put("value", json.encodeToJsonElement(ListSerializer(NotificationDTO.serializer()), value.value))
                    }
                is MuxyResult.Ok ->
                    buildJsonObject {
                        put("type", "ok")
                    }
            }
        output.encodeJsonElement(element)
    }

    override fun deserialize(decoder: Decoder): MuxyResult {
        val input =
            (decoder as? JsonDecoder)
                ?: error("MuxyResultSerializer only supports JSON")
        val obj = input.decodeJsonElement().jsonObject
        val type = obj.getValue("type").jsonPrimitive.content
        if (type == "ok") return MuxyResult.Ok
        val value: JsonElement = obj.getValue("value")
        val json = input.json
        return when (type) {
            "projects" -> MuxyResult.Projects(json.decodeFromJsonElement(ListSerializer(ProjectDTO.serializer()), value))
            "worktrees" -> MuxyResult.Worktrees(json.decodeFromJsonElement(ListSerializer(WorktreeDTO.serializer()), value))
            "workspace" -> MuxyResult.Workspace(json.decodeFromJsonElement(WorkspaceDTO.serializer(), value))
            "tab" -> MuxyResult.Tab(json.decodeFromJsonElement(TabDTO.serializer(), value))
            "terminalContent" -> MuxyResult.TerminalContent(json.decodeFromJsonElement(TerminalContentDTO.serializer(), value))
            "terminalCells" -> MuxyResult.TerminalCells(json.decodeFromJsonElement(TerminalCellsDTO.serializer(), value))
            "deviceInfo" -> MuxyResult.DeviceInfo(json.decodeFromJsonElement(DeviceInfoDTO.serializer(), value))
            "pairing" -> MuxyResult.Pairing(json.decodeFromJsonElement(PairingResultDTO.serializer(), value))
            "paneOwner" -> MuxyResult.PaneOwner(json.decodeFromJsonElement(PaneOwnerDTO.serializer(), value))
            "vcsStatus" -> MuxyResult.VCSStatus(json.decodeFromJsonElement(VCSStatusDTO.serializer(), value))
            "vcsBranches" -> MuxyResult.VCSBranches(json.decodeFromJsonElement(VCSBranchesDTO.serializer(), value))
            "vcsPRCreated" -> MuxyResult.VCSPRCreated(json.decodeFromJsonElement(VCSCreatePRResultDTO.serializer(), value))
            "projectLogo" -> MuxyResult.ProjectLogo(json.decodeFromJsonElement(ProjectLogoDTO.serializer(), value))
            "notifications" -> MuxyResult.Notifications(json.decodeFromJsonElement(ListSerializer(NotificationDTO.serializer()), value))
            else -> error("Unknown MuxyResult type: $type")
        }
    }
}

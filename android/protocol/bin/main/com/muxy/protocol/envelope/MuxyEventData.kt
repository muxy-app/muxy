package com.muxy.protocol.envelope

import com.muxy.protocol.dto.DeviceThemeEventDTO
import com.muxy.protocol.dto.NotificationDTO
import com.muxy.protocol.dto.PaneOwnershipEventDTO
import com.muxy.protocol.dto.ProjectDTO
import com.muxy.protocol.dto.TabChangeEventDTO
import com.muxy.protocol.dto.TerminalOutputEventDTO
import com.muxy.protocol.dto.WorkspaceDTO
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

@Serializable(with = MuxyEventDataSerializer::class)
sealed class MuxyEventData {
    data class Workspace(val value: WorkspaceDTO) : MuxyEventData()

    data class Tab(val value: TabChangeEventDTO) : MuxyEventData()

    data class TerminalOutput(val value: TerminalOutputEventDTO) : MuxyEventData()

    data class TerminalSnapshot(val value: TerminalOutputEventDTO) : MuxyEventData()

    data class Notification(val value: NotificationDTO) : MuxyEventData()

    data class Projects(val value: List<ProjectDTO>) : MuxyEventData()

    data class PaneOwnership(val value: PaneOwnershipEventDTO) : MuxyEventData()

    data class DeviceTheme(val value: DeviceThemeEventDTO) : MuxyEventData()
}

object MuxyEventDataSerializer : KSerializer<MuxyEventData> {
    override val descriptor: SerialDescriptor = buildClassSerialDescriptor("MuxyEventData")

    override fun serialize(
        encoder: Encoder,
        value: MuxyEventData,
    ) {
        val output =
            (encoder as? JsonEncoder)
                ?: error("MuxyEventDataSerializer only supports JSON")
        val json = output.json
        val element =
            when (value) {
                is MuxyEventData.Workspace ->
                    buildJsonObject {
                        put("type", "workspace")
                        put("value", json.encodeToJsonElement(WorkspaceDTO.serializer(), value.value))
                    }
                is MuxyEventData.Tab ->
                    buildJsonObject {
                        put("type", "tab")
                        put("value", json.encodeToJsonElement(TabChangeEventDTO.serializer(), value.value))
                    }
                is MuxyEventData.TerminalOutput ->
                    buildJsonObject {
                        put("type", "terminalOutput")
                        put("value", json.encodeToJsonElement(TerminalOutputEventDTO.serializer(), value.value))
                    }
                is MuxyEventData.TerminalSnapshot ->
                    buildJsonObject {
                        put("type", "terminalSnapshot")
                        put("value", json.encodeToJsonElement(TerminalOutputEventDTO.serializer(), value.value))
                    }
                is MuxyEventData.Notification ->
                    buildJsonObject {
                        put("type", "notification")
                        put("value", json.encodeToJsonElement(NotificationDTO.serializer(), value.value))
                    }
                is MuxyEventData.Projects ->
                    buildJsonObject {
                        put("type", "projects")
                        put("value", json.encodeToJsonElement(ListSerializer(ProjectDTO.serializer()), value.value))
                    }
                is MuxyEventData.PaneOwnership ->
                    buildJsonObject {
                        put("type", "paneOwnership")
                        put("value", json.encodeToJsonElement(PaneOwnershipEventDTO.serializer(), value.value))
                    }
                is MuxyEventData.DeviceTheme ->
                    buildJsonObject {
                        put("type", "deviceTheme")
                        put("value", json.encodeToJsonElement(DeviceThemeEventDTO.serializer(), value.value))
                    }
            }
        output.encodeJsonElement(element)
    }

    override fun deserialize(decoder: Decoder): MuxyEventData {
        val input =
            (decoder as? JsonDecoder)
                ?: error("MuxyEventDataSerializer only supports JSON")
        val obj = input.decodeJsonElement().jsonObject
        val type = obj.getValue("type").jsonPrimitive.content
        val value: JsonElement = obj.getValue("value")
        val json = input.json
        return when (type) {
            "workspace" -> MuxyEventData.Workspace(json.decodeFromJsonElement(WorkspaceDTO.serializer(), value))
            "tab" -> MuxyEventData.Tab(json.decodeFromJsonElement(TabChangeEventDTO.serializer(), value))
            "terminalOutput" -> MuxyEventData.TerminalOutput(json.decodeFromJsonElement(TerminalOutputEventDTO.serializer(), value))
            "terminalSnapshot" -> MuxyEventData.TerminalSnapshot(json.decodeFromJsonElement(TerminalOutputEventDTO.serializer(), value))
            "notification" -> MuxyEventData.Notification(json.decodeFromJsonElement(NotificationDTO.serializer(), value))
            "projects" -> MuxyEventData.Projects(json.decodeFromJsonElement(ListSerializer(ProjectDTO.serializer()), value))
            "paneOwnership" -> MuxyEventData.PaneOwnership(json.decodeFromJsonElement(PaneOwnershipEventDTO.serializer(), value))
            "deviceTheme" -> MuxyEventData.DeviceTheme(json.decodeFromJsonElement(DeviceThemeEventDTO.serializer(), value))
            else -> error("Unknown MuxyEventData type: $type")
        }
    }
}

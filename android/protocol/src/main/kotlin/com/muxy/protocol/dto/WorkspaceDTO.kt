package com.muxy.protocol.dto

import kotlinx.serialization.Contextual
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.buildClassSerialDescriptor
import kotlinx.serialization.descriptors.element
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonEncoder
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import java.util.UUID

@Serializable
data class WorkspaceDTO(
    val projectID: @Contextual UUID,
    val worktreeID: @Contextual UUID,
    val focusedAreaID: @Contextual UUID? = null,
    val root: SplitNodeDTO,
)

@Serializable
enum class SplitDirectionDTO {
    @SerialName("horizontal") HORIZONTAL,
    @SerialName("vertical") VERTICAL,
}

@Serializable
enum class SplitPositionDTO {
    @SerialName("first") FIRST,
    @SerialName("second") SECOND,
}

@Serializable(with = SplitNodeSerializer::class)
sealed class SplitNodeDTO {
    data class TabArea(val tabArea: TabAreaDTO) : SplitNodeDTO()
    data class Split(val split: SplitBranchDTO) : SplitNodeDTO()
}

object SplitNodeSerializer : KSerializer<SplitNodeDTO> {
    override val descriptor: SerialDescriptor = buildClassSerialDescriptor("SplitNodeDTO") {
        element<String>("type")
    }

    override fun serialize(encoder: Encoder, value: SplitNodeDTO) {
        val output = (encoder as? JsonEncoder)
            ?: error("SplitNodeSerializer only supports JSON")
        val element = when (value) {
            is SplitNodeDTO.TabArea -> buildJsonObject {
                put("type", "tabArea")
                put("tabArea", output.json.encodeToJsonElement(TabAreaDTO.serializer(), value.tabArea))
            }
            is SplitNodeDTO.Split -> buildJsonObject {
                put("type", "split")
                put("split", output.json.encodeToJsonElement(SplitBranchDTO.serializer(), value.split))
            }
        }
        output.encodeJsonElement(element)
    }

    override fun deserialize(decoder: Decoder): SplitNodeDTO {
        val input = (decoder as? JsonDecoder)
            ?: error("SplitNodeSerializer only supports JSON")
        val obj = input.decodeJsonElement().jsonObject
        return when (val type = obj.getValue("type").jsonPrimitive.content) {
            "tabArea" -> SplitNodeDTO.TabArea(
                input.json.decodeFromJsonElement(TabAreaDTO.serializer(), obj.getValue("tabArea"))
            )
            "split" -> SplitNodeDTO.Split(
                input.json.decodeFromJsonElement(SplitBranchDTO.serializer(), obj.getValue("split"))
            )
            else -> error("Unknown SplitNodeDTO type: $type")
        }
    }
}

@Serializable
data class SplitBranchDTO(
    val id: @Contextual UUID,
    val direction: SplitDirectionDTO,
    val ratio: Double,
    val first: SplitNodeDTO,
    val second: SplitNodeDTO,
)

@Serializable
data class TabAreaDTO(
    val id: @Contextual UUID,
    val projectPath: String,
    val tabs: List<TabDTO>,
    val activeTabID: @Contextual UUID? = null,
)

@Serializable
data class TabDTO(
    val id: @Contextual UUID,
    val kind: TabKindDTO,
    val title: String,
    val isPinned: Boolean,
    val paneID: @Contextual UUID? = null,
)

@Serializable
enum class TabKindDTO {
    @SerialName("terminal") TERMINAL,
    @SerialName("vcs") VCS,
    @SerialName("editor") EDITOR,
    @SerialName("diffViewer") DIFF_VIEWER,
}

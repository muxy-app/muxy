package com.muxy.protocol

import com.muxy.protocol.codec.MuxyCodec
import com.muxy.protocol.dto.AuthenticateDeviceParams
import com.muxy.protocol.dto.GitFileStatusDTO
import com.muxy.protocol.dto.NotificationDTO
import com.muxy.protocol.dto.NotificationSourceDTO
import com.muxy.protocol.dto.PairingResultDTO
import com.muxy.protocol.dto.PaneOwnerDTO
import com.muxy.protocol.dto.ProjectDTO
import com.muxy.protocol.dto.SelectProjectParams
import com.muxy.protocol.dto.SplitBranchDTO
import com.muxy.protocol.dto.SplitDirectionDTO
import com.muxy.protocol.dto.SplitNodeDTO
import com.muxy.protocol.dto.TabAreaDTO
import com.muxy.protocol.dto.TabDTO
import com.muxy.protocol.dto.TabKindDTO
import com.muxy.protocol.dto.TerminalInputParams
import com.muxy.protocol.dto.VCSStatusDTO
import com.muxy.protocol.dto.WorkspaceDTO
import com.muxy.protocol.dto.WorktreeDTO
import com.muxy.protocol.envelope.MuxyError
import com.muxy.protocol.envelope.MuxyEventData
import com.muxy.protocol.envelope.MuxyEventKind
import com.muxy.protocol.envelope.MuxyMessage
import com.muxy.protocol.envelope.MuxyMethod
import com.muxy.protocol.envelope.MuxyParams
import com.muxy.protocol.envelope.MuxyRequest
import com.muxy.protocol.envelope.MuxyResponse
import com.muxy.protocol.envelope.MuxyResult
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant
import java.util.UUID

class MuxyCodecTest {
    private val timestamp: Instant = Instant.parse("2026-05-01T12:34:56Z")
    private val projectID = UUID.fromString("11111111-1111-1111-1111-111111111111")
    private val worktreeID = UUID.fromString("22222222-2222-2222-2222-222222222222")
    private val areaID = UUID.fromString("33333333-3333-3333-3333-333333333333")
    private val tabID = UUID.fromString("44444444-4444-4444-4444-444444444444")
    private val paneID = UUID.fromString("55555555-5555-5555-5555-555555555555")
    private val deviceID = UUID.fromString("66666666-6666-6666-6666-666666666666")
    private val clientID = UUID.fromString("77777777-7777-7777-7777-777777777777")

    @Test
    fun `request envelope serializes type and payload`() {
        val request =
            MuxyRequest(
                id = "req-1",
                method = MuxyMethod.LIST_PROJECTS,
                params = null,
            )
        val message = MuxyMessage.Request(request)
        val text = MuxyCodec.encode(message)
        val obj = Json.parseToJsonElement(text).jsonObject
        assertEquals("request", obj.getValue("type").jsonPrimitive.content)
        val payload = obj.getValue("payload").jsonObject
        assertEquals("req-1", payload.getValue("id").jsonPrimitive.content)
        assertEquals("listProjects", payload.getValue("method").jsonPrimitive.content)
        assertNull(payload["params"])
    }

    @Test
    fun `response envelope encodes ok result without value key`() {
        val response = MuxyResponse(id = "r-1", result = MuxyResult.Ok)
        val message = MuxyMessage.Response(response)
        val obj = Json.parseToJsonElement(MuxyCodec.encode(message)).jsonObject
        assertEquals("response", obj.getValue("type").jsonPrimitive.content)
        val payload = obj.getValue("payload").jsonObject
        val result = payload.getValue("result").jsonObject
        assertEquals("ok", result.getValue("type").jsonPrimitive.content)
        assertNull(result["value"])
    }

    @Test
    fun `response envelope decodes ok shape`() {
        val raw =
            """
            {"type":"response","payload":{"id":"r-2","result":{"type":"ok"}}}
            """.trimIndent()
        val message = MuxyCodec.decode(raw)
        assertTrue(message is MuxyMessage.Response)
        val response = (message as MuxyMessage.Response).value
        assertEquals("r-2", response.id)
        assertTrue(response.result is MuxyResult.Ok)
        assertNull(response.error)
    }

    @Test
    fun `error response decodes code and message`() {
        val raw =
            """
            {"type":"response","payload":{"id":"r-3","error":{"code":401,"message":"Authentication required"}}}
            """.trimIndent()
        val message = MuxyCodec.decode(raw)
        val response = (message as MuxyMessage.Response).value
        assertEquals(MuxyError(code = 401, message = "Authentication required"), response.error)
        assertNull(response.result)
    }

    @Test
    fun `params shape uses inner type and value keys`() {
        val request =
            MuxyRequest(
                id = "p-1",
                method = MuxyMethod.SELECT_PROJECT,
                params = MuxyParams.SelectProject(SelectProjectParams(projectID)),
            )
        val message = MuxyMessage.Request(request)
        val obj = Json.parseToJsonElement(MuxyCodec.encode(message)).jsonObject
        val params = obj.getValue("payload").jsonObject.getValue("params").jsonObject
        assertEquals("selectProject", params.getValue("type").jsonPrimitive.content)
        val value = params.getValue("value").jsonObject
        assertEquals(projectID.toString().uppercase(), value.getValue("projectID").jsonPrimitive.content)
    }

    @Test
    fun `terminalInput params encodes bytes as base64 string`() {
        val bytes = byteArrayOf(0x68, 0x69, 0x0A)
        val params = MuxyParams.TerminalInput(TerminalInputParams(paneID, bytes))
        val request = MuxyRequest(id = "t-1", method = MuxyMethod.TERMINAL_INPUT, params = params)
        val text = MuxyCodec.encode(MuxyMessage.Request(request))
        val obj = Json.parseToJsonElement(text).jsonObject
        val value =
            obj.getValue("payload").jsonObject
                .getValue("params").jsonObject
                .getValue("value").jsonObject
        val encoded = value.getValue("bytes").jsonPrimitive.content
        assertEquals("aGkK", encoded)

        val roundTrip = MuxyCodec.decode(text)
        val decoded = ((roundTrip as MuxyMessage.Request).value.params as MuxyParams.TerminalInput).value
        assertTrue(decoded.bytes.contentEquals(bytes))
    }

    @Test
    fun `auth params round-trip preserves uppercase UUID and string token`() {
        val params =
            MuxyParams.AuthenticateDevice(
                AuthenticateDeviceParams(
                    deviceID = deviceID,
                    deviceName = "Pixel 8",
                    token = "ZmFrZS10b2tlbg==",
                ),
            )
        val request = MuxyRequest(id = "a-1", method = MuxyMethod.AUTHENTICATE_DEVICE, params = params)
        val text = MuxyCodec.encode(MuxyMessage.Request(request))
        val value =
            Json.parseToJsonElement(text).jsonObject
                .getValue("payload").jsonObject
                .getValue("params").jsonObject
                .getValue("value").jsonObject
        assertEquals(deviceID.toString().uppercase(), value.getValue("deviceID").jsonPrimitive.content)
        assertEquals("ZmFrZS10b2tlbg==", value.getValue("token").jsonPrimitive.content)
    }

    @Test
    fun `pairing result roundtrip preserves theme palette`() {
        val pairing =
            PairingResultDTO(
                clientID = clientID,
                deviceName = "Mac",
                themeFg = 0xFFEEDDu,
                themeBg = 0x101010u,
                themePalette = listOf(0u, 1u, 2u, 3u, 0xFFFFFFu),
            )
        val response = MuxyResponse(id = "x-1", result = MuxyResult.Pairing(pairing))
        val text = MuxyCodec.encode(MuxyMessage.Response(response))
        val decoded = MuxyCodec.decode(text)
        val out = ((decoded as MuxyMessage.Response).value.result as MuxyResult.Pairing).value
        assertEquals(pairing, out)
    }

    @Test
    fun `splitNode tabArea wire shape uses keyed inner field`() {
        val tabArea =
            TabAreaDTO(
                id = areaID,
                projectPath = "/p",
                tabs =
                    listOf(
                        TabDTO(id = tabID, kind = TabKindDTO.TERMINAL, title = "zsh", isPinned = false, paneID = paneID),
                    ),
                activeTabID = tabID,
            )
        val node: SplitNodeDTO = SplitNodeDTO.TabArea(tabArea)
        val text = MuxyCodec.json.encodeToString(SplitNodeDTO.serializer(), node)
        val obj = Json.parseToJsonElement(text).jsonObject
        assertEquals("tabArea", obj.getValue("type").jsonPrimitive.content)
        assertNotNull(obj["tabArea"])
        assertNull(obj["value"])
    }

    @Test
    fun `splitNode split wire shape uses keyed inner field`() {
        val branch =
            SplitBranchDTO(
                id = areaID,
                direction = SplitDirectionDTO.HORIZONTAL,
                ratio = 0.5,
                first = SplitNodeDTO.TabArea(emptyArea(areaID)),
                second = SplitNodeDTO.TabArea(emptyArea(UUID.fromString("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA"))),
            )
        val node: SplitNodeDTO = SplitNodeDTO.Split(branch)
        val text = MuxyCodec.json.encodeToString(SplitNodeDTO.serializer(), node)
        val obj = Json.parseToJsonElement(text).jsonObject
        assertEquals("split", obj.getValue("type").jsonPrimitive.content)
        assertNotNull(obj["split"])
    }

    @Test
    fun `workspace round-trips through nested splits`() {
        val ws =
            WorkspaceDTO(
                projectID = projectID,
                worktreeID = worktreeID,
                focusedAreaID = areaID,
                root =
                    SplitNodeDTO.Split(
                        SplitBranchDTO(
                            id = areaID,
                            direction = SplitDirectionDTO.VERTICAL,
                            ratio = 0.4,
                            first = SplitNodeDTO.TabArea(emptyArea(areaID)),
                            second = SplitNodeDTO.TabArea(emptyArea(tabID)),
                        ),
                    ),
            )
        val text = MuxyCodec.json.encodeToString(WorkspaceDTO.serializer(), ws)
        val parsed = MuxyCodec.json.decodeFromString(WorkspaceDTO.serializer(), text)
        assertEquals(ws, parsed)
    }

    @Test
    fun `paneOwner mac uses single-key wire shape`() {
        val owner: PaneOwnerDTO = PaneOwnerDTO.Mac(deviceName = "MacBook")
        val text = MuxyCodec.json.encodeToString(PaneOwnerDTO.serializer(), owner)
        val obj = Json.parseToJsonElement(text).jsonObject
        assertNotNull(obj["mac"])
        assertEquals("MacBook", obj.getValue("mac").jsonObject.getValue("deviceName").jsonPrimitive.content)
        val parsed = MuxyCodec.json.decodeFromString(PaneOwnerDTO.serializer(), text)
        assertEquals(owner, parsed)
    }

    @Test
    fun `paneOwner remote serializes UUID and name`() {
        val owner: PaneOwnerDTO = PaneOwnerDTO.Remote(deviceID = deviceID, deviceName = "Pixel")
        val text = MuxyCodec.json.encodeToString(PaneOwnerDTO.serializer(), owner)
        val obj = Json.parseToJsonElement(text).jsonObject
        val inner = obj.getValue("remote").jsonObject
        assertEquals(deviceID.toString().uppercase(), inner.getValue("deviceID").jsonPrimitive.content)
        assertEquals("Pixel", inner.getValue("deviceName").jsonPrimitive.content)
        val parsed = MuxyCodec.json.decodeFromString(PaneOwnerDTO.serializer(), text)
        assertEquals(owner, parsed)
    }

    @Test
    fun `pane ownership event decodes from wire shape`() {
        val raw =
            """
            {"type":"event","payload":{"event":"paneOwnershipChanged","data":{"type":"paneOwnership","value":{"paneID":"55555555-5555-5555-5555-555555555555","owner":{"remote":{"deviceID":"66666666-6666-6666-6666-666666666666","deviceName":"Phone"}}}}}}
            """.trimIndent()
        val message = MuxyCodec.decode(raw)
        val event = (message as MuxyMessage.Event).value
        assertEquals(MuxyEventKind.PANE_OWNERSHIP_CHANGED, event.event)
        val data = event.data as MuxyEventData.PaneOwnership
        assertEquals(paneID, data.value.paneID)
        assertEquals(PaneOwnerDTO.Remote(deviceID, "Phone"), data.value.owner)
    }

    @Test
    fun `terminal output event decodes base64 bytes`() {
        val raw =
            """
            {"type":"event","payload":{"event":"terminalOutput","data":{"type":"terminalOutput","value":{"paneID":"55555555-5555-5555-5555-555555555555","bytes":"aGkK"}}}}
            """.trimIndent()
        val message = MuxyCodec.decode(raw)
        val data = (message as MuxyMessage.Event).value.data as MuxyEventData.TerminalOutput
        assertTrue(data.value.bytes.contentEquals(byteArrayOf(0x68, 0x69, 0x0A)))
    }

    @Test
    fun `terminal snapshot event matches output event shape`() {
        val raw =
            """
            {"type":"event","payload":{"event":"terminalSnapshot","data":{"type":"terminalSnapshot","value":{"paneID":"55555555-5555-5555-5555-555555555555","bytes":"aGVsbG8="}}}}
            """.trimIndent()
        val event = (MuxyCodec.decode(raw) as MuxyMessage.Event).value
        assertEquals(MuxyEventKind.TERMINAL_SNAPSHOT, event.event)
        val data = event.data as MuxyEventData.TerminalSnapshot
        assertTrue(data.value.bytes.contentEquals("hello".toByteArray()))
    }

    @Test
    fun `theme changed event maps to deviceTheme data case`() {
        val raw =
            """
            {"type":"event","payload":{"event":"themeChanged","data":{"type":"deviceTheme","value":{"fg":16777215,"bg":0,"palette":[0,1,2]}}}}
            """.trimIndent()
        val event = (MuxyCodec.decode(raw) as MuxyMessage.Event).value
        assertEquals(MuxyEventKind.THEME_CHANGED, event.event)
        val data = event.data as MuxyEventData.DeviceTheme
        assertEquals(0xFFFFFFu, data.value.fg)
        assertEquals(0u, data.value.bg)
        assertEquals(listOf(0u, 1u, 2u), data.value.palette)
    }

    @Test
    fun `notification source aiProvider uses _0 unlabeled key`() {
        val notification =
            NotificationDTO(
                id = UUID.randomUUID(),
                paneID = paneID,
                projectID = projectID,
                worktreeID = worktreeID,
                areaID = areaID,
                tabID = tabID,
                source = NotificationSourceDTO.AiProvider("openai"),
                title = "Title",
                body = "Body",
                timestamp = timestamp,
                isRead = false,
            )
        val text = MuxyCodec.json.encodeToString(NotificationDTO.serializer(), notification)
        val source = Json.parseToJsonElement(text).jsonObject.getValue("source").jsonObject
        val provider = source.getValue("aiProvider").jsonObject.getValue("_0").jsonPrimitive.content
        assertEquals("openai", provider)
        val parsed = MuxyCodec.json.decodeFromString(NotificationDTO.serializer(), text)
        assertEquals(notification, parsed)
    }

    @Test
    fun `notification source osc and socket are object-empty`() {
        val sources = listOf(NotificationSourceDTO.Osc, NotificationSourceDTO.Socket)
        for (source in sources) {
            val text = MuxyCodec.json.encodeToString(NotificationSourceDTO.serializer(), source)
            val obj = Json.parseToJsonElement(text).jsonObject
            val key = if (source is NotificationSourceDTO.Osc) "osc" else "socket"
            assertNotNull(obj[key])
            val parsed = MuxyCodec.json.decodeFromString(NotificationSourceDTO.serializer(), text)
            assertEquals(source, parsed)
        }
    }

    @Test
    fun `worktree without canBeRemoved field decodes to negation of isPrimary`() {
        val raw =
            """
            {"id":"22222222-2222-2222-2222-222222222222","name":"main","path":"/p","isPrimary":true,"createdAt":"2026-05-01T12:34:56Z"}
            """.trimIndent()
        val parsed = MuxyCodec.json.decodeFromString(WorktreeDTO.serializer(), raw)
        assertEquals(false, parsed.canBeRemoved)
        assertEquals(true, parsed.isPrimary)
        assertNull(parsed.branch)
    }

    @Test
    fun `worktree always emits canBeRemoved on encode`() {
        val tree =
            WorktreeDTO(
                id = worktreeID,
                name = "main",
                path = "/p",
                branch = "main",
                isPrimary = true,
                canBeRemoved = false,
                createdAt = timestamp,
            )
        val text = MuxyCodec.json.encodeToString(WorktreeDTO.serializer(), tree)
        val obj = Json.parseToJsonElement(text).jsonObject
        assertEquals(false, obj.getValue("canBeRemoved").jsonPrimitive.boolean)
        assertEquals("main", obj.getValue("branch").jsonPrimitive.content)
    }

    @Test
    fun `worktree omits null branch on encode`() {
        val tree =
            WorktreeDTO(
                id = worktreeID,
                name = "main",
                path = "/p",
                branch = null,
                isPrimary = true,
                createdAt = timestamp,
            )
        val text = MuxyCodec.json.encodeToString(WorktreeDTO.serializer(), tree)
        val obj = Json.parseToJsonElement(text).jsonObject
        assertNull(obj["branch"])
    }

    @Test
    fun `project DTO round-trips with optional logo and color`() {
        val project =
            ProjectDTO(
                id = projectID,
                name = "Muxy",
                path = "/Users/me/dev/muxy",
                sortOrder = 0,
                createdAt = timestamp,
                icon = "folder",
                logo = "L",
                iconColor = "blue",
            )
        val text = MuxyCodec.json.encodeToString(ProjectDTO.serializer(), project)
        val parsed = MuxyCodec.json.decodeFromString(ProjectDTO.serializer(), text)
        assertEquals(project, parsed)
    }

    @Test
    fun `vcs status with pull request decodes`() {
        val raw =
            """
            {
              "branch":"main",
              "aheadCount":1,
              "behindCount":0,
              "hasUpstream":true,
              "stagedFiles":[{"path":"a.swift","status":"modified","isUntracked":false}],
              "changedFiles":[],
              "defaultBranch":"main",
              "pullRequest":{"url":"https://github.com/x/y/pull/1","number":1,"state":"open","isDraft":false,"baseBranch":"main"}
            }
            """.trimIndent()
        val parsed = MuxyCodec.json.decodeFromString(VCSStatusDTO.serializer(), raw)
        assertEquals(1, parsed.aheadCount)
        assertEquals(0, parsed.behindCount)
        assertEquals(GitFileStatusDTO.MODIFIED, parsed.stagedFiles.first().status)
        assertEquals("https://github.com/x/y/pull/1", parsed.pullRequest?.url)
    }

    @Test
    fun `subscribe params encodes events as method strings`() {
        val request =
            MuxyRequest(
                id = "s-1",
                method = MuxyMethod.SUBSCRIBE,
                params =
                    MuxyParams.Subscribe(
                        com.muxy.protocol.dto.SubscribeParams(
                            events = listOf(MuxyEventKind.WORKSPACE_CHANGED, MuxyEventKind.TERMINAL_OUTPUT),
                        ),
                    ),
            )
        val text = MuxyCodec.encode(MuxyMessage.Request(request))
        val list =
            Json.parseToJsonElement(text).jsonObject
                .getValue("payload").jsonObject
                .getValue("params").jsonObject
                .getValue("value").jsonObject
                .getValue("events").jsonArray
        assertEquals("workspaceChanged", list[0].jsonPrimitive.content)
        assertEquals("terminalOutput", list[1].jsonPrimitive.content)
    }

    @Test
    fun `unknown keys in incoming payloads are ignored`() {
        val raw =
            """
            {"type":"response","payload":{"id":"u-1","result":{"type":"ok"},"extraField":42}}
            """.trimIndent()
        val message = MuxyCodec.decode(raw)
        assertTrue((message as MuxyMessage.Response).value.result is MuxyResult.Ok)
    }

    private fun emptyArea(id: UUID) =
        TabAreaDTO(
            id = id,
            projectPath = "/x",
            tabs = emptyList(),
            activeTabID = null,
        )
}

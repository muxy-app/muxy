package com.muxy.net

import com.muxy.protocol.codec.MuxyCodec
import com.muxy.protocol.dto.PaneOwnerDTO
import com.muxy.protocol.dto.PairingResultDTO
import com.muxy.protocol.dto.PaneOwnershipEventDTO
import com.muxy.protocol.dto.ProjectDTO
import com.muxy.protocol.dto.TerminalInputParams
import com.muxy.protocol.dto.TerminalOutputEventDTO
import com.muxy.protocol.envelope.MuxyError
import com.muxy.protocol.envelope.MuxyEvent
import com.muxy.protocol.envelope.MuxyEventData
import com.muxy.protocol.envelope.MuxyEventKind
import com.muxy.protocol.envelope.MuxyMessage
import com.muxy.protocol.envelope.MuxyMethod
import com.muxy.protocol.envelope.MuxyParams
import com.muxy.protocol.envelope.MuxyRequest
import com.muxy.protocol.envelope.MuxyResponse
import com.muxy.protocol.envelope.MuxyResult
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.withTimeoutOrNull
import okhttp3.OkHttpClient
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.time.Instant
import java.util.UUID
import java.util.concurrent.TimeUnit
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

@OptIn(DelicateCoroutinesApi::class)
class MuxyClientTest {
    private lateinit var server: FakeMuxyServer
    private lateinit var client: MuxyClient
    private val deviceID = UUID.fromString("AAAAAAAA-1111-1111-1111-111111111111")
    private val token = "ZmFrZS10b2tlbg=="
    private val clientID = UUID.fromString("BBBBBBBB-2222-2222-2222-222222222222")

    @Before
    fun setUp() {
        server = FakeMuxyServer()
        client = MuxyClient(
            httpClient = OkHttpClient.Builder().readTimeout(0, TimeUnit.MILLISECONDS).build(),
            credentialsProvider = object : DeviceCredentialsProvider {
                override suspend fun load(): DeviceCredentials =
                    DeviceCredentials(deviceID, token)
            },
        )
    }

    @After
    fun tearDown() {
        client.close()
        server.close()
    }

    @Test
    fun `authenticateDevice success transitions to Connected`() = runBlocking {
        server.start { incoming ->
            val request = (incoming as MuxyMessage.Request).value
            when (request.method) {
                MuxyMethod.AUTHENTICATE_DEVICE -> {
                    val pairing = PairingResultDTO(clientID = clientID, deviceName = "Mac")
                    server.broadcast(
                        MuxyMessage.Response(
                            MuxyResponse(id = request.id, result = MuxyResult.Pairing(pairing)),
                        ),
                    )
                }
                else -> Unit
            }
        }

        client.connect(ConnectionTarget(server.host, server.port, "Pixel"))

        withTimeout(2.seconds) {
            client.state.first { it is ConnectionState.Connected }
        }
        assertEquals(clientID, client.myClientID.value)
    }

    @Test
    fun `401 unauthorized triggers pairing flow then connects`() = runBlocking {
        var seenAuth = false
        server.start { incoming ->
            val request = (incoming as MuxyMessage.Request).value
            when (request.method) {
                MuxyMethod.AUTHENTICATE_DEVICE -> {
                    seenAuth = true
                    server.broadcast(
                        MuxyMessage.Response(
                            MuxyResponse(id = request.id, error = MuxyError(code = 401, message = "auth")),
                        ),
                    )
                }
                MuxyMethod.PAIR_DEVICE -> {
                    val pairing = PairingResultDTO(clientID = clientID, deviceName = "Mac")
                    server.broadcast(
                        MuxyMessage.Response(
                            MuxyResponse(id = request.id, result = MuxyResult.Pairing(pairing)),
                        ),
                    )
                }
                else -> Unit
            }
        }

        client.connect(ConnectionTarget(server.host, server.port, "Pixel"))

        withTimeout(2.seconds) {
            client.state.first { it is ConnectionState.Connected }
        }
        assertTrue(seenAuth)
        assertEquals(clientID, client.myClientID.value)
    }

    @Test
    fun `403 pairingDenied surfaces approval-denied failure`() = runBlocking {
        server.start { incoming ->
            val request = (incoming as MuxyMessage.Request).value
            when (request.method) {
                MuxyMethod.AUTHENTICATE_DEVICE -> server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(id = request.id, error = MuxyError(code = 401, message = "auth")),
                    ),
                )
                MuxyMethod.PAIR_DEVICE -> server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(id = request.id, error = MuxyError(code = 403, message = "denied")),
                    ),
                )
                else -> Unit
            }
        }

        client.connect(ConnectionTarget(server.host, server.port, "Pixel"))

        val state = withTimeout(2.seconds) {
            client.state.first { it is ConnectionState.Failed }
        } as ConnectionState.Failed
        assertEquals("Approval denied on Mac", state.issue.message)
    }

    @Test
    fun `terminalInput is fire-and-forget and does not register pending request`() = runBlocking {
        server.start { incoming ->
            val request = (incoming as MuxyMessage.Request).value
            if (request.method == MuxyMethod.AUTHENTICATE_DEVICE) {
                server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(
                            id = request.id,
                            result = MuxyResult.Pairing(PairingResultDTO(clientID, "Mac")),
                        ),
                    ),
                )
            }
        }
        client.connect(ConnectionTarget(server.host, server.port, "Pixel"))
        withTimeout(2.seconds) { client.state.first { it is ConnectionState.Connected } }

        val paneID = UUID.randomUUID()
        client.sendFireAndForget(
            method = MuxyMethod.TERMINAL_INPUT,
            params = MuxyParams.TerminalInput(
                TerminalInputParams(paneID = paneID, bytes = "hello".toByteArray()),
            ),
        )
        delay(150)
        val terminalInputs = server.receivedMessages().filter { msg ->
            msg is MuxyMessage.Request && msg.value.method == MuxyMethod.TERMINAL_INPUT
        }
        assertEquals(1, terminalInputs.size)
    }

    @Test
    fun `RPC round-trip listProjects returns projects payload`() = runBlocking {
        val project = ProjectDTO(
            id = UUID.randomUUID(),
            name = "Muxy",
            path = "/x",
            sortOrder = 0,
            createdAt = Instant.parse("2026-05-01T10:00:00Z"),
        )
        server.start { incoming ->
            val request = (incoming as MuxyMessage.Request).value
            when (request.method) {
                MuxyMethod.AUTHENTICATE_DEVICE -> server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(
                            id = request.id,
                            result = MuxyResult.Pairing(PairingResultDTO(clientID, "Mac")),
                        ),
                    ),
                )
                MuxyMethod.LIST_PROJECTS -> server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(
                            id = request.id,
                            result = MuxyResult.Projects(listOf(project)),
                        ),
                    ),
                )
                else -> Unit
            }
        }
        client.connect(ConnectionTarget(server.host, server.port, "Pixel"))
        withTimeout(2.seconds) { client.state.first { it is ConnectionState.Connected } }

        val response = client.send(MuxyMethod.LIST_PROJECTS)
        assertNotNull(response)
        val result = response!!.result as MuxyResult.Projects
        assertEquals(listOf(project), result.value)
    }

    @Test
    fun `RPC times out and returns 408 when server never responds`() = runBlocking {
        server.start { incoming ->
            val request = (incoming as MuxyMessage.Request).value
            if (request.method == MuxyMethod.AUTHENTICATE_DEVICE) {
                server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(
                            id = request.id,
                            result = MuxyResult.Pairing(PairingResultDTO(clientID, "Mac")),
                        ),
                    ),
                )
            }
        }
        client.connect(ConnectionTarget(server.host, server.port, "Pixel"))
        withTimeout(2.seconds) { client.state.first { it is ConnectionState.Connected } }

        val response = client.send(MuxyMethod.LIST_PROJECTS, timeout = 100.milliseconds)
        assertNotNull(response)
        assertEquals(408, response!!.error!!.code)
    }

    @Test
    fun `paneOwnershipChanged event updates paneOwners map`() = runBlocking {
        server.start { incoming ->
            val request = (incoming as MuxyMessage.Request).value
            if (request.method == MuxyMethod.AUTHENTICATE_DEVICE) {
                server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(
                            id = request.id,
                            result = MuxyResult.Pairing(PairingResultDTO(clientID, "Mac")),
                        ),
                    ),
                )
            }
        }
        client.connect(ConnectionTarget(server.host, server.port, "Pixel"))
        withTimeout(2.seconds) { client.state.first { it is ConnectionState.Connected } }

        val paneID = UUID.randomUUID()
        val owner = PaneOwnerDTO.Remote(deviceID = clientID, deviceName = "Pixel")
        server.broadcast(
            MuxyMessage.Event(
                MuxyEvent(
                    event = MuxyEventKind.PANE_OWNERSHIP_CHANGED,
                    data = MuxyEventData.PaneOwnership(
                        PaneOwnershipEventDTO(paneID = paneID, owner = owner),
                    ),
                ),
            ),
        )

        withTimeout(2.seconds) {
            client.paneOwners.first { it.containsKey(paneID) }
        }
        assertEquals(owner, client.paneOwners.value[paneID])
        assertTrue(client.paneIsOwnedBySelf(paneID))
    }

    @Test
    fun `terminalOutput and terminalSnapshot both flow through terminalBytes for that pane`() = runBlocking {
        server.start { incoming ->
            val request = (incoming as MuxyMessage.Request).value
            if (request.method == MuxyMethod.AUTHENTICATE_DEVICE) {
                server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(
                            id = request.id,
                            result = MuxyResult.Pairing(PairingResultDTO(clientID, "Mac")),
                        ),
                    ),
                )
            }
        }
        client.connect(ConnectionTarget(server.host, server.port, "Pixel"))
        withTimeout(2.seconds) { client.state.first { it is ConnectionState.Connected } }

        val paneID = UUID.randomUUID()
        val collected = mutableListOf<ByteArray>()
        val collector = GlobalScope.launch(start = CoroutineStart.UNDISPATCHED) {
            client.terminalBytes(paneID).collect { bytes -> collected.add(bytes) }
        }

        server.broadcast(
            MuxyMessage.Event(
                MuxyEvent(
                    event = MuxyEventKind.TERMINAL_SNAPSHOT,
                    data = MuxyEventData.TerminalSnapshot(
                        TerminalOutputEventDTO(paneID, "snap".toByteArray()),
                    ),
                ),
            ),
        )
        server.broadcast(
            MuxyMessage.Event(
                MuxyEvent(
                    event = MuxyEventKind.TERMINAL_OUTPUT,
                    data = MuxyEventData.TerminalOutput(
                        TerminalOutputEventDTO(paneID, "live".toByteArray()),
                    ),
                ),
            ),
        )

        withTimeoutOrNull(2.seconds) {
            while (collected.size < 2) delay(20)
        }
        collector.cancel()
        assertEquals(2, collected.size)
        assertEquals("snap", String(collected[0]))
        assertEquals("live", String(collected[1]))
    }

    @Test
    fun `verifyConnectionOrReconnect re-authenticates and clears paneOwners`() = runBlocking {
        var authCount = 0
        server.start { incoming ->
            val request = (incoming as MuxyMessage.Request).value
            if (request.method == MuxyMethod.AUTHENTICATE_DEVICE) {
                authCount += 1
                server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(
                            id = request.id,
                            result = MuxyResult.Pairing(PairingResultDTO(clientID, "Mac")),
                        ),
                    ),
                )
            }
        }
        client.connect(ConnectionTarget(server.host, server.port, "Pixel"))
        withTimeout(2.seconds) { client.state.first { it is ConnectionState.Connected } }

        val paneID = UUID.randomUUID()
        server.broadcast(
            MuxyMessage.Event(
                MuxyEvent(
                    event = MuxyEventKind.PANE_OWNERSHIP_CHANGED,
                    data = MuxyEventData.PaneOwnership(
                        PaneOwnershipEventDTO(paneID, PaneOwnerDTO.Remote(clientID, "Pixel")),
                    ),
                ),
            ),
        )
        withTimeout(2.seconds) { client.paneOwners.first { it.containsKey(paneID) } }

        client.verifyConnectionOrReconnect()

        withTimeout(2.seconds) {
            while (authCount < 2) delay(20)
        }
        assertTrue(client.paneOwners.value.isEmpty())
    }

    @Test
    fun `disconnect cancels pending requests with cancellation error`() = runBlocking {
        server.start { incoming ->
            val request = (incoming as MuxyMessage.Request).value
            if (request.method == MuxyMethod.AUTHENTICATE_DEVICE) {
                server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(
                            id = request.id,
                            result = MuxyResult.Pairing(PairingResultDTO(clientID, "Mac")),
                        ),
                    ),
                )
            }
        }
        client.connect(ConnectionTarget(server.host, server.port, "Pixel"))
        withTimeout(2.seconds) { client.state.first { it is ConnectionState.Connected } }

        val responseDeferred = GlobalScope.async {
            client.send(MuxyMethod.LIST_PROJECTS, timeout = 5.seconds)
        }
        delay(100)
        client.disconnect()
        val response = withTimeout(2.seconds) { responseDeferred.await() }
        assertNotNull(response)
        assertEquals(499, response!!.error!!.code)
    }
}

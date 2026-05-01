package com.muxy.net

import com.muxy.protocol.dto.MarkNotificationReadParams
import com.muxy.protocol.dto.NotificationDTO
import com.muxy.protocol.dto.NotificationSourceDTO
import com.muxy.protocol.dto.PairingResultDTO
import com.muxy.protocol.envelope.MuxyError
import com.muxy.protocol.envelope.MuxyMessage
import com.muxy.protocol.envelope.MuxyMethod
import com.muxy.protocol.envelope.MuxyParams
import com.muxy.protocol.envelope.MuxyRequest
import com.muxy.protocol.envelope.MuxyResponse
import com.muxy.protocol.envelope.MuxyResult
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import okhttp3.OkHttpClient
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.time.Instant
import java.util.UUID
import java.util.concurrent.TimeUnit
import kotlin.time.Duration.Companion.seconds

class MuxyClientNotificationsTest {
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
                override suspend fun load(): DeviceCredentials = DeviceCredentials(deviceID, token)
            },
        )
    }

    @After
    fun tearDown() {
        client.close()
        server.close()
    }

    private fun fakeNotification(
        id: UUID = UUID.randomUUID(),
        timestamp: Instant = Instant.parse("2026-01-01T00:00:00Z"),
        isRead: Boolean = false,
        title: String = "Title",
    ) = NotificationDTO(
        id = id,
        paneID = UUID.randomUUID(),
        projectID = UUID.randomUUID(),
        worktreeID = UUID.randomUUID(),
        areaID = UUID.randomUUID(),
        tabID = UUID.randomUUID(),
        source = NotificationSourceDTO.Osc,
        title = title,
        body = "body",
        timestamp = timestamp,
        isRead = isRead,
    )

    private suspend fun startAndConnect(responder: (MuxyRequest) -> Unit) {
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
            } else {
                responder(request)
            }
        }
        client.connect(ConnectionTarget(server.host, server.port, "Pixel"))
        withTimeout(2.seconds) { client.state.first { it is ConnectionState.Connected } }
    }

    @Test
    fun `refreshNotifications stores list sorted by timestamp desc`() = runBlocking {
        val older = fakeNotification(timestamp = Instant.parse("2026-01-01T00:00:00Z"), title = "older")
        val newer = fakeNotification(timestamp = Instant.parse("2026-02-01T00:00:00Z"), title = "newer")
        startAndConnect { request ->
            if (request.method == MuxyMethod.LIST_NOTIFICATIONS) {
                server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(
                            id = request.id,
                            result = MuxyResult.Notifications(listOf(older, newer)),
                        ),
                    ),
                )
            }
        }
        val ok = client.refreshNotifications()
        assertTrue(ok)
        val result = client.notifications.value
        assertEquals(2, result.size)
        assertEquals("newer", result.first().title)
        assertEquals("older", result.last().title)
    }

    @Test
    fun `refreshNotifications returns false on server error`() = runBlocking {
        startAndConnect { request ->
            if (request.method == MuxyMethod.LIST_NOTIFICATIONS) {
                server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(id = request.id, error = MuxyError(code = 500, message = "boom")),
                    ),
                )
            }
        }
        assertFalse(client.refreshNotifications())
        assertTrue(client.notifications.value.isEmpty())
    }

    @Test
    fun `markNotificationRead flips isRead in cached list`() = runBlocking {
        val target = fakeNotification(title = "target", isRead = false)
        val other = fakeNotification(title = "other", isRead = false)
        var seenMarkID: UUID? = null
        startAndConnect { request ->
            when (request.method) {
                MuxyMethod.LIST_NOTIFICATIONS -> server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(
                            id = request.id,
                            result = MuxyResult.Notifications(listOf(target, other)),
                        ),
                    ),
                )
                MuxyMethod.MARK_NOTIFICATION_READ -> {
                    val params = (request.params as MuxyParams.MarkNotificationRead).value
                    seenMarkID = (params as MarkNotificationReadParams).notificationID
                    server.broadcast(
                        MuxyMessage.Response(MuxyResponse(id = request.id, result = MuxyResult.Ok)),
                    )
                }
                else -> Unit
            }
        }
        assertTrue(client.refreshNotifications())
        assertTrue(client.markNotificationRead(target.id))
        assertEquals(target.id, seenMarkID)
        val updated = client.notifications.value
        val updatedTarget = updated.first { it.id == target.id }
        val updatedOther = updated.first { it.id == other.id }
        assertTrue(updatedTarget.isRead)
        assertFalse(updatedOther.isRead)
    }
}

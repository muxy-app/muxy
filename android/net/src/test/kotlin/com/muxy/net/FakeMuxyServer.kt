package com.muxy.net

import com.muxy.protocol.codec.MuxyCodec
import com.muxy.protocol.envelope.MuxyMessage
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okio.ByteString
import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.flow.MutableSharedFlow

class FakeMuxyServer : AutoCloseable {
    private val server = MockWebServer()
    private val incoming = MutableSharedFlow<MuxyMessage>(extraBufferCapacity = 64)
    private val openLatch = CountDownLatch(1)
    private val sockets = ConcurrentLinkedQueue<WebSocket>()
    private val received = ConcurrentLinkedQueue<MuxyMessage>()
    private var responder: ((MuxyMessage) -> Unit)? = null

    val host: String get() = server.hostName
    val port: Int get() = server.port

    fun start(maxConnections: Int = 4, responder: (MuxyMessage) -> Unit) {
        this.responder = responder
        repeat(maxConnections) {
            server.enqueue(
                MockResponse().withWebSocketUpgrade(
                    object : WebSocketListener() {
                        override fun onOpen(webSocket: WebSocket, response: Response) {
                            sockets.add(webSocket)
                            openLatch.countDown()
                        }

                        override fun onMessage(webSocket: WebSocket, text: String) {
                            val message = MuxyCodec.decode(text)
                            received.add(message)
                            this@FakeMuxyServer.responder?.invoke(message)
                        }

                        override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
                            val message = MuxyCodec.decode(bytes.utf8())
                            received.add(message)
                            this@FakeMuxyServer.responder?.invoke(message)
                        }

                        override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                            sockets.remove(webSocket)
                        }
                    },
                ),
            )
        }
        server.start()
    }

    fun awaitOpen(timeoutMs: Long = 2_000) {
        require(openLatch.await(timeoutMs, TimeUnit.MILLISECONDS)) { "WebSocket never opened" }
    }

    fun broadcast(message: MuxyMessage) {
        sockets.forEach { it.send(MuxyCodec.encode(message)) }
    }

    fun closeAll() {
        sockets.forEach { it.cancel() }
        sockets.clear()
    }

    fun receivedMessages(): List<MuxyMessage> = received.toList()

    override fun close() {
        sockets.forEach { it.close(1000, null) }
        server.shutdown()
    }
}

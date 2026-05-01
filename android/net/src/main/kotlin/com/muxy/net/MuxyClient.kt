package com.muxy.net

import com.muxy.protocol.codec.MuxyCodec
import com.muxy.protocol.dto.AuthenticateDeviceParams
import com.muxy.protocol.dto.CloseAreaParams
import com.muxy.protocol.dto.CloseTabParams
import com.muxy.protocol.dto.CreateTabParams
import com.muxy.protocol.dto.FocusAreaParams
import com.muxy.protocol.dto.GetProjectLogoParams
import com.muxy.protocol.dto.GetWorkspaceParams
import com.muxy.protocol.dto.ListWorktreesParams
import com.muxy.protocol.dto.MarkNotificationReadParams
import com.muxy.protocol.dto.NotificationDTO
import com.muxy.protocol.dto.PairDeviceParams
import com.muxy.protocol.dto.PaneOwnerDTO
import com.muxy.protocol.dto.ProjectDTO
import com.muxy.protocol.dto.ReleasePaneParams
import com.muxy.protocol.dto.SelectProjectParams
import com.muxy.protocol.dto.SelectTabParams
import com.muxy.protocol.dto.SelectWorktreeParams
import com.muxy.protocol.dto.SplitAreaParams
import com.muxy.protocol.dto.SplitDirectionDTO
import com.muxy.protocol.dto.SplitPositionDTO
import com.muxy.protocol.dto.TakeOverPaneParams
import com.muxy.protocol.dto.TerminalInputParams
import com.muxy.protocol.dto.TerminalResizeParams
import com.muxy.protocol.dto.WorkspaceDTO
import com.muxy.protocol.dto.WorktreeDTO
import com.muxy.protocol.envelope.MuxyError
import com.muxy.protocol.envelope.MuxyEvent
import com.muxy.protocol.envelope.MuxyEventData
import com.muxy.protocol.envelope.MuxyMessage
import com.muxy.protocol.envelope.MuxyMethod
import com.muxy.protocol.envelope.MuxyParams
import com.muxy.protocol.envelope.MuxyRequest
import com.muxy.protocol.envelope.MuxyResponse
import com.muxy.protocol.envelope.MuxyResult
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeoutOrNull
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import java.io.IOException
import java.util.Base64
import java.util.UUID
import java.util.concurrent.TimeUnit
import kotlin.random.Random
import kotlin.time.Duration

interface DeviceCredentialsProvider {
    suspend fun load(): DeviceCredentials
}

data class DeviceCredentials(val deviceID: UUID, val token: String)

class MuxyClient(
    private val httpClient: OkHttpClient = defaultClient(),
    private val credentialsProvider: DeviceCredentialsProvider,
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO,
    private val backoff: BackoffPolicy = BackoffPolicy.exponentialJitter(),
    private val now: () -> Long = { System.currentTimeMillis() },
) : AutoCloseable {
    private val parent = SupervisorJob()
    private val scope = CoroutineScope(ioDispatcher + parent)

    private val _state = MutableStateFlow<ConnectionState>(ConnectionState.Idle)
    val state: StateFlow<ConnectionState> = _state.asStateFlow()

    private val _events = MutableSharedFlow<MuxyEvent>(extraBufferCapacity = 64)
    val events: SharedFlow<MuxyEvent> = _events.asSharedFlow()

    private val _paneOwners = MutableStateFlow<Map<UUID, PaneOwnerDTO>>(emptyMap())
    val paneOwners: StateFlow<Map<UUID, PaneOwnerDTO>> = _paneOwners.asStateFlow()

    private val _myClientID = MutableStateFlow<UUID?>(null)
    val myClientID: StateFlow<UUID?> = _myClientID.asStateFlow()

    private val _deviceTheme = MutableStateFlow<DeviceTheme?>(null)
    val deviceTheme: StateFlow<DeviceTheme?> = _deviceTheme.asStateFlow()

    private val _activeProjectID = MutableStateFlow<UUID?>(null)
    val activeProjectID: StateFlow<UUID?> = _activeProjectID.asStateFlow()

    private val _projects = MutableStateFlow<List<ProjectDTO>>(emptyList())
    val projects: StateFlow<List<ProjectDTO>> = _projects.asStateFlow()

    private val _projectLogos = MutableStateFlow<Map<UUID, ByteArray>>(emptyMap())
    val projectLogos: StateFlow<Map<UUID, ByteArray>> = _projectLogos.asStateFlow()

    private val _projectWorktrees = MutableStateFlow<Map<UUID, List<WorktreeDTO>>>(emptyMap())
    val projectWorktrees: StateFlow<Map<UUID, List<WorktreeDTO>>> = _projectWorktrees.asStateFlow()

    private val _workspace = MutableStateFlow<WorkspaceDTO?>(null)
    val workspace: StateFlow<WorkspaceDTO?> = _workspace.asStateFlow()

    private val _notifications = MutableStateFlow<List<NotificationDTO>>(emptyList())
    val notifications: StateFlow<List<NotificationDTO>> = _notifications.asStateFlow()

    val log: DiagnosticLog = DiagnosticLog()

    private val pendingMutex = Mutex()
    private val pending = mutableMapOf<String, CompletableDeferred<MuxyResponse>>()
    private val pendingMethods = mutableMapOf<String, MuxyMethod>()

    private var webSocket: WebSocket? = null
    private var currentTarget: ConnectionTarget? = null
    private var connectJob: Job? = null

    private val reconnectMutex = Mutex()
    private var isReconnecting: Boolean = false

    @Volatile
    var isBackgrounded: Boolean = false
        private set

    fun paneIsOwnedBySelf(paneID: UUID): Boolean {
        val mine = _myClientID.value ?: return false
        val owner = _paneOwners.value[paneID] ?: return false
        return owner is PaneOwnerDTO.Remote && owner.deviceID == mine
    }

    fun terminalBytes(paneID: UUID): Flow<ByteArray> =
        _events
            .filter { event ->
                val data = event.data
                (data is MuxyEventData.TerminalOutput && data.value.paneID == paneID) ||
                    (data is MuxyEventData.TerminalSnapshot && data.value.paneID == paneID)
            }
            .map { event ->
                when (val data = event.data) {
                    is MuxyEventData.TerminalOutput -> data.value.bytes
                    is MuxyEventData.TerminalSnapshot -> data.value.bytes
                    else -> ByteArray(0)
                }
            }

    fun setBackgrounded(value: Boolean) {
        isBackgrounded = value
    }

    fun connect(target: ConnectionTarget) {
        connectJob?.cancel()
        currentTarget = target
        log.append("Connect requested for ${target.deviceName} at ${target.host}:${target.port}")
        _paneOwners.value = emptyMap()
        _activeProjectID.value = null
        _workspace.value = null
        _projects.value = emptyList()
        _projectLogos.value = emptyMap()
        _projectWorktrees.value = emptyMap()
        _state.value = ConnectionState.Connecting(target)
        _notifications.value = emptyList()
        connectJob = scope.launch {
            openSocket(target)
            authenticateOrPair(target)
        }
    }

    fun disconnect() {
        log.append("Disconnected")
        connectJob?.cancel()
        connectJob = null
        webSocket?.close(1000, null)
        webSocket = null
        currentTarget = null
        _state.value = ConnectionState.Idle
        scope.launch {
            cancelAllPending(MuxyError(code = 499, message = "Cancelled"))
        }
        _paneOwners.value = emptyMap()
        _deviceTheme.value = null
        _myClientID.value = null
        _activeProjectID.value = null
        _workspace.value = null
        _projects.value = emptyList()
        _projectLogos.value = emptyMap()
        _projectWorktrees.value = emptyMap()
        _notifications.value = emptyList()
    }

    suspend fun send(
        method: MuxyMethod,
        params: MuxyParams? = null,
        timeout: Duration = MuxyTimeouts.forMethod(method),
    ): MuxyResponse? {
        check(method !in MuxyTimeouts.voidMethods) {
            "${method.name} is fire-and-forget; use sendFireAndForget"
        }
        val socket = webSocket ?: run {
            log.append("Send ${method.name} skipped: no socket")
            return null
        }
        val id = UUID.randomUUID().toString()
        val request = MuxyRequest(id = id, method = method, params = params)
        val message = MuxyMessage.Request(request)
        val text = MuxyCodec.encode(message)
        val deferred = CompletableDeferred<MuxyResponse>()
        pendingMutex.withLock {
            pending[id] = deferred
            pendingMethods[id] = method
        }
        log.append("→ ${method.name} [$id]")
        val sent = socket.send(text)
        if (!sent) {
            pendingMutex.withLock {
                pending.remove(id)
                pendingMethods.remove(id)
            }
            log.append("× ${method.name} [$id] queue rejected")
            return null
        }
        return withTimeoutOrNull(timeout) { deferred.await() } ?: run {
            pendingMutex.withLock {
                pending.remove(id)
                pendingMethods.remove(id)
            }
            log.append("× ${method.name} [$id] timed out")
            MuxyResponse(id = id, error = MuxyError(code = 408, message = "Timeout"))
        }
    }

    fun sendFireAndForget(method: MuxyMethod, params: MuxyParams) {
        val socket = webSocket ?: return
        val request = MuxyRequest(id = UUID.randomUUID().toString(), method = method, params = params)
        socket.send(MuxyCodec.encode(MuxyMessage.Request(request)))
    }

    fun sendTerminalInput(paneID: UUID, bytes: ByteArray) {
        if (bytes.isEmpty()) return
        sendFireAndForget(
            MuxyMethod.TERMINAL_INPUT,
            MuxyParams.TerminalInput(TerminalInputParams(paneID = paneID, bytes = bytes)),
        )
    }

    suspend fun takeOverPane(paneID: UUID, cols: UInt, rows: UInt) {
        send(
            MuxyMethod.TAKE_OVER_PANE,
            MuxyParams.TakeOverPane(TakeOverPaneParams(paneID = paneID, cols = cols, rows = rows)),
        )
    }

    suspend fun releasePane(paneID: UUID) {
        send(MuxyMethod.RELEASE_PANE, MuxyParams.ReleasePane(ReleasePaneParams(paneID = paneID)))
    }

    suspend fun resizeTerminal(paneID: UUID, cols: UInt, rows: UInt) {
        send(
            MuxyMethod.TERMINAL_RESIZE,
            MuxyParams.TerminalResize(TerminalResizeParams(paneID = paneID, cols = cols, rows = rows)),
        )
    }

    suspend fun refreshProjects(): Boolean {
        val response = send(MuxyMethod.LIST_PROJECTS) ?: return false
        if (response.error != null) return false
        val result = response.result as? MuxyResult.Projects ?: return false
        _projects.value = result.value
        for (project in result.value) {
            if (project.logo != null) fetchProjectLogo(project.id)
            refreshWorktrees(project.id)
        }
        return true
    }

    suspend fun fetchProjectLogo(projectID: UUID): Boolean {
        if (_projectLogos.value.containsKey(projectID)) return true
        val response = send(
            MuxyMethod.GET_PROJECT_LOGO,
            MuxyParams.GetProjectLogo(GetProjectLogoParams(projectID = projectID)),
        ) ?: return false
        val result = response.result as? MuxyResult.ProjectLogo ?: return false
        val data = runCatching { Base64.getDecoder().decode(result.value.pngData) }.getOrNull()
            ?: return false
        _projectLogos.value = _projectLogos.value + (projectID to data)
        return true
    }

    suspend fun selectProject(projectID: UUID): Boolean {
        _activeProjectID.value = projectID
        _workspace.value = null
        _paneOwners.value = emptyMap()
        val response = send(
            MuxyMethod.SELECT_PROJECT,
            MuxyParams.SelectProject(SelectProjectParams(projectID = projectID)),
        ) ?: return false
        if (response.error != null) return false
        return refreshWorkspace(projectID)
    }

    suspend fun refreshWorktrees(projectID: UUID): Boolean {
        val response = send(
            MuxyMethod.LIST_WORKTREES,
            MuxyParams.ListWorktrees(ListWorktreesParams(projectID = projectID)),
        ) ?: return false
        if (response.error != null) return false
        val result = response.result as? MuxyResult.Worktrees ?: return false
        _projectWorktrees.value = _projectWorktrees.value + (projectID to result.value)
        return true
    }

    suspend fun refreshWorkspace(projectID: UUID): Boolean {
        val response = send(
            MuxyMethod.GET_WORKSPACE,
            MuxyParams.GetWorkspace(GetWorkspaceParams(projectID = projectID)),
        ) ?: return false
        if (response.error != null) return false
        val result = response.result as? MuxyResult.Workspace ?: return false
        _workspace.value = result.value
        return true
    }

    suspend fun selectWorktree(projectID: UUID, worktreeID: UUID): Boolean {
        val response = send(
            MuxyMethod.SELECT_WORKTREE,
            MuxyParams.SelectWorktree(SelectWorktreeParams(projectID = projectID, worktreeID = worktreeID)),
        ) ?: return false
        if (response.error != null) return false
        return refreshWorkspace(projectID)
    }

    suspend fun createTab(projectID: UUID, areaID: UUID? = null) {
        send(
            MuxyMethod.CREATE_TAB,
            MuxyParams.CreateTab(CreateTabParams(projectID = projectID, areaID = areaID)),
        )
        refreshWorkspace(projectID)
    }

    suspend fun closeTab(projectID: UUID, areaID: UUID, tabID: UUID) {
        send(
            MuxyMethod.CLOSE_TAB,
            MuxyParams.CloseTab(CloseTabParams(projectID = projectID, areaID = areaID, tabID = tabID)),
        )
        refreshWorkspace(projectID)
    }

    suspend fun selectTab(projectID: UUID, areaID: UUID, tabID: UUID) {
        send(
            MuxyMethod.SELECT_TAB,
            MuxyParams.SelectTab(SelectTabParams(projectID = projectID, areaID = areaID, tabID = tabID)),
        )
        refreshWorkspace(projectID)
    }

    suspend fun focusArea(projectID: UUID, areaID: UUID) {
        send(
            MuxyMethod.FOCUS_AREA,
            MuxyParams.FocusArea(FocusAreaParams(projectID = projectID, areaID = areaID)),
        )
        refreshWorkspace(projectID)
    }

    suspend fun splitArea(
        projectID: UUID,
        areaID: UUID,
        direction: SplitDirectionDTO,
        position: SplitPositionDTO,
    ) {
        send(
            MuxyMethod.SPLIT_AREA,
            MuxyParams.SplitArea(
                SplitAreaParams(projectID = projectID, areaID = areaID, direction = direction, position = position),
            ),
        )
        refreshWorkspace(projectID)
    }

    suspend fun closeArea(projectID: UUID, areaID: UUID) {
        send(
            MuxyMethod.CLOSE_AREA,
            MuxyParams.CloseArea(CloseAreaParams(projectID = projectID, areaID = areaID)),
        )
        refreshWorkspace(projectID)
    }

    suspend fun refreshNotifications(): Boolean {
        val response = send(MuxyMethod.LIST_NOTIFICATIONS) ?: return false
        if (response.error != null) return false
        val result = response.result as? MuxyResult.Notifications ?: return false
        _notifications.value = result.value.sortedByDescending { it.timestamp }
        return true
    }

    suspend fun markNotificationRead(notificationID: UUID): Boolean {
        val response = send(
            MuxyMethod.MARK_NOTIFICATION_READ,
            MuxyParams.MarkNotificationRead(MarkNotificationReadParams(notificationID = notificationID)),
        ) ?: return false
        if (response.error != null) return false
        _notifications.value = _notifications.value.map { existing ->
            if (existing.id == notificationID) existing.copy(isRead = true) else existing
        }
        return true
    }

    fun verifyConnectionOrReconnect() {
        val target = currentTarget ?: return
        scope.launch { reconnectSilently(target) }
    }

    private suspend fun reconnectSilently(target: ConnectionTarget) {
        reconnectMutex.withLock {
            if (isReconnecting) return
            isReconnecting = true
        }
        try {
            log.append("Silent reconnect to ${target.host}:${target.port}")
            _paneOwners.value = emptyMap()
            webSocket?.cancel()
            openSocket(target, attemptCount = 1)
            authenticateOrPair(target, silent = true)
            val activeID = _activeProjectID.value
            if (activeID != null && _state.value is ConnectionState.Connected) {
                send(MuxyMethod.SELECT_PROJECT, MuxyParams.SelectProject(SelectProjectParams(projectID = activeID)))
                refreshWorkspace(activeID)
            }
        } finally {
            reconnectMutex.withLock { isReconnecting = false }
        }
    }

    private suspend fun openSocket(target: ConnectionTarget, attemptCount: Int = 0) {
        val url = "ws://${target.host}:${target.port}"
        log.append("Opening WebSocket to ${target.host}:${target.port}")
        val request = Request.Builder().url(url).build()
        val listener = SocketListener()
        webSocket = httpClient.newWebSocket(request, listener)
        if (attemptCount > 0) {
            val delayMs = backoff.delayForAttempt(attemptCount)
            if (delayMs > 0) delay(delayMs)
        }
    }

    private suspend fun authenticateOrPair(target: ConnectionTarget, silent: Boolean = false) {
        val credentials = credentialsProvider.load()
        if (!silent) _state.value = ConnectionState.Authenticating(target)
        val authParams = MuxyParams.AuthenticateDevice(
            AuthenticateDeviceParams(
                deviceID = credentials.deviceID,
                deviceName = target.deviceName,
                token = credentials.token,
            ),
        )
        val authResponse = send(MuxyMethod.AUTHENTICATE_DEVICE, authParams)
        if (authResponse == null) {
            if (!silent) failed("Could not reach device", "Authenticating device", target)
            return
        }
        val authError = authResponse.error
        if (authError == null) {
            applyPairing(authResponse.result, target)
            return
        }
        if (authError.code != 401) {
            failed(
                message = "Authentication failed",
                operation = "Authenticating device",
                target = target,
                requestMethod = MuxyMethod.AUTHENTICATE_DEVICE.name,
                requestID = authResponse.id,
                responseError = authError,
            )
            return
        }
        if (!silent) _state.value = ConnectionState.AwaitingApproval(target)
        val pairParams = MuxyParams.PairDevice(
            PairDeviceParams(
                deviceID = credentials.deviceID,
                deviceName = target.deviceName,
                token = credentials.token,
            ),
        )
        val pairResponse = send(MuxyMethod.PAIR_DEVICE, pairParams)
        if (pairResponse == null) {
            failed("Could not finish pairing", "Pairing device", target)
            return
        }
        val pairError = pairResponse.error
        if (pairError != null) {
            val message = if (pairError.code == 403) "Approval denied on Mac" else "Could not finish pairing"
            failed(
                message = message,
                operation = "Pairing device",
                target = target,
                requestMethod = MuxyMethod.PAIR_DEVICE.name,
                requestID = pairResponse.id,
                responseError = pairError,
            )
            return
        }
        applyPairing(pairResponse.result, target)
    }

    private fun applyPairing(result: MuxyResult?, target: ConnectionTarget) {
        if (result is MuxyResult.Pairing) {
            _myClientID.value = result.value.clientID
            val fg = result.value.themeFg
            val bg = result.value.themeBg
            if (fg != null && bg != null) {
                _deviceTheme.value = DeviceTheme(
                    fg = fg,
                    bg = bg,
                    palette = result.value.themePalette ?: emptyList(),
                )
            }
            log.append("Authenticated as client ${result.value.clientID}")
            _state.value = ConnectionState.Connected(target)
            return
        }
        failed(
            message = "Authentication failed",
            operation = "Authenticating device",
            target = target,
            requestMethod = MuxyMethod.AUTHENTICATE_DEVICE.name,
            requestID = null,
            responseError = null,
        )
    }

    private fun failed(
        message: String,
        operation: String,
        target: ConnectionTarget?,
        requestMethod: String? = null,
        requestID: String? = null,
        responseError: MuxyError? = null,
        underlyingError: String? = null,
    ) {
        log.append("Failure during $operation: $message")
        if (_state.value is ConnectionState.Idle) return
        val issue = ConnectionIssue(
            message = message,
            operation = operation,
            timestamp = DiagnosticLog.formatter.format(java.time.Instant.ofEpochMilli(now())),
            target = target,
            requestMethod = requestMethod,
            requestID = requestID,
            responseError = responseError,
            underlyingError = underlyingError,
            recentLog = log.lastN(25),
        )
        _state.value = ConnectionState.Failed(issue, target)
    }

    private fun handleIncomingText(text: String) {
        val message = try {
            MuxyCodec.decode(text)
        } catch (t: Throwable) {
            log.append("Failed to decode incoming message: ${t.message}")
            return
        }
        when (message) {
            is MuxyMessage.Response -> handleResponse(message.value)
            is MuxyMessage.Event -> handleEvent(message.value)
            is MuxyMessage.Request -> Unit
        }
    }

    private fun handleResponse(response: MuxyResponse) {
        val deferred: CompletableDeferred<MuxyResponse>?
        val method: MuxyMethod?
        synchronized(pending) {
            deferred = pending.remove(response.id)
            method = pendingMethods.remove(response.id)
        }
        if (deferred != null) {
            log.append("← ${method?.name ?: "?"} [${response.id}] ${summary(response)}")
            deferred.complete(response)
        }
    }

    private fun handleEvent(event: MuxyEvent) {
        when (val data = event.data) {
            is MuxyEventData.PaneOwnership -> {
                _paneOwners.value = _paneOwners.value + (data.value.paneID to data.value.owner)
            }
            is MuxyEventData.DeviceTheme -> {
                _deviceTheme.value = DeviceTheme(
                    fg = data.value.fg,
                    bg = data.value.bg,
                    palette = data.value.palette ?: emptyList(),
                )
            }
            else -> Unit
        }
        scope.launch { _events.emit(event) }
    }

    private suspend fun cancelAllPending(error: MuxyError) {
        val snapshot = pendingMutex.withLock {
            val copy = pending.toMap()
            pending.clear()
            pendingMethods.clear()
            copy
        }
        for ((id, deferred) in snapshot) {
            deferred.complete(MuxyResponse(id = id, error = error))
        }
    }

    override fun close() {
        disconnect()
        scope.cancel()
        httpClient.dispatcher.executorService.shutdown()
        httpClient.connectionPool.evictAll()
    }

    private inner class SocketListener : WebSocketListener() {
        override fun onOpen(webSocket: WebSocket, response: Response) {
            log.append("WebSocket open")
        }

        override fun onMessage(webSocket: WebSocket, text: String) {
            handleIncomingText(text)
        }

        override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
            handleIncomingText(bytes.utf8())
        }

        override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
            log.append("WebSocket failure: ${t.message}")
            scope.launch {
                cancelAllPending(MuxyError(code = 499, message = "Connection lost"))
            }
            if (isBackgrounded) {
                log.append("Suppressing error: backgrounded")
                return
            }
            failed(
                message = "Connection lost",
                operation = "WebSocket",
                target = currentTarget,
                underlyingError = t.message,
            )
        }

        override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
            webSocket.close(1000, null)
        }

        override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
            log.append("WebSocket closed: $code $reason")
        }
    }

    private fun summary(response: MuxyResponse): String {
        val error = response.error
        if (error != null) return "error ${error.code} ${error.message}"
        return when (val result = response.result) {
            null -> "ok"
            is MuxyResult.Ok -> "ok"
            is MuxyResult.Projects -> "projects(${result.value.size})"
            is MuxyResult.Worktrees -> "worktrees(${result.value.size})"
            is MuxyResult.Workspace -> "workspace"
            is MuxyResult.Tab -> "tab"
            is MuxyResult.TerminalContent -> "terminalContent"
            is MuxyResult.TerminalCells -> "terminalCells"
            is MuxyResult.DeviceInfo -> "deviceInfo"
            is MuxyResult.Pairing -> "pairing"
            is MuxyResult.PaneOwner -> "paneOwner"
            is MuxyResult.VCSStatus -> "vcsStatus"
            is MuxyResult.VCSBranches -> "vcsBranches"
            is MuxyResult.VCSPRCreated -> "vcsPRCreated"
            is MuxyResult.ProjectLogo -> "projectLogo"
            is MuxyResult.Notifications -> "notifications(${result.value.size})"
        }
    }

    companion object {
        fun defaultClient(): OkHttpClient = OkHttpClient.Builder()
            .pingInterval(20, TimeUnit.SECONDS)
            .readTimeout(0, TimeUnit.MILLISECONDS)
            .build()
    }
}

class BackoffPolicy(
    private val baseMs: Long,
    private val maxMs: Long,
    private val jitterMs: Long,
    private val random: Random = Random.Default,
) {
    fun delayForAttempt(attempt: Int): Long {
        if (attempt <= 0) return 0
        val capped = minOf(maxMs, baseMs shl (attempt - 1).coerceAtMost(20))
        val jitter = if (jitterMs <= 0) 0 else random.nextLong(jitterMs)
        return capped + jitter
    }

    companion object {
        fun exponentialJitter(
            baseMs: Long = 250,
            maxMs: Long = 10_000,
            jitterMs: Long = 250,
        ): BackoffPolicy = BackoffPolicy(baseMs, maxMs, jitterMs)
    }
}

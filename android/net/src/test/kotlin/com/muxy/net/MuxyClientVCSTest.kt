package com.muxy.net

import com.muxy.protocol.dto.GitFileDTO
import com.muxy.protocol.dto.GitFileStatusDTO
import com.muxy.protocol.dto.PairingResultDTO
import com.muxy.protocol.dto.VCSBranchesDTO
import com.muxy.protocol.dto.VCSCreatePRResultDTO
import com.muxy.protocol.dto.VCSStatusDTO
import com.muxy.protocol.dto.WorktreeDTO
import com.muxy.protocol.envelope.MuxyError
import com.muxy.protocol.envelope.MuxyMessage
import com.muxy.protocol.envelope.MuxyMethod
import com.muxy.protocol.envelope.MuxyRequest
import com.muxy.protocol.envelope.MuxyResponse
import com.muxy.protocol.envelope.MuxyResult
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import okhttp3.OkHttpClient
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.time.Instant
import java.util.UUID
import java.util.concurrent.TimeUnit
import kotlin.time.Duration.Companion.seconds

class MuxyClientVCSTest {
    private lateinit var server: FakeMuxyServer
    private lateinit var client: MuxyClient
    private val deviceID = UUID.fromString("AAAAAAAA-1111-1111-1111-111111111111")
    private val token = "ZmFrZS10b2tlbg=="
    private val clientID = UUID.fromString("BBBBBBBB-2222-2222-2222-222222222222")
    private val projectID = UUID.fromString("CCCCCCCC-3333-3333-3333-333333333333")

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
    fun `fetchVCSStatus returns parsed payload on success`() = runBlocking {
        val staged = listOf(GitFileDTO(path = "a.txt", status = GitFileStatusDTO.MODIFIED))
        val changed = listOf(GitFileDTO(path = "b.txt", status = GitFileStatusDTO.UNTRACKED, isUntracked = true))
        val expected = VCSStatusDTO(
            branch = "main",
            aheadCount = 1,
            behindCount = 0,
            hasUpstream = true,
            stagedFiles = staged,
            changedFiles = changed,
        )
        startAndConnect { request ->
            if (request.method == MuxyMethod.GET_VCS_STATUS) {
                server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(id = request.id, result = MuxyResult.VCSStatus(expected)),
                    ),
                )
            }
        }
        assertEquals(expected, client.fetchVCSStatus(projectID))
    }

    @Test
    fun `fetchVCSStatus returns null on server error`() = runBlocking {
        startAndConnect { request ->
            if (request.method == MuxyMethod.GET_VCS_STATUS) {
                server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(id = request.id, error = MuxyError(code = 500, message = "boom")),
                    ),
                )
            }
        }
        assertNull(client.fetchVCSStatus(projectID))
    }

    @Test
    fun `vcsCommit succeeds when server returns ok`() = runBlocking {
        var seenCommit = false
        startAndConnect { request ->
            if (request.method == MuxyMethod.VCS_COMMIT) {
                seenCommit = true
                server.broadcast(
                    MuxyMessage.Response(MuxyResponse(id = request.id, result = MuxyResult.Ok)),
                )
            }
        }
        client.vcsCommit(projectID, "msg", stageAll = false)
        assertTrue(seenCommit)
    }

    @Test
    fun `stageFiles throws Server error when Mac responds with error`() = runBlocking {
        startAndConnect { request ->
            if (request.method == MuxyMethod.VCS_STAGE_FILES) {
                server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(id = request.id, error = MuxyError(code = 500, message = "nope")),
                    ),
                )
            }
        }
        val ex = assertThrows(VCSClientError.Server::class.java) {
            runBlocking { client.stageFiles(projectID, listOf("a.txt")) }
        }
        assertEquals("nope", ex.message)
    }

    @Test
    fun `listBranches returns parsed payload`() = runBlocking {
        val expected = VCSBranchesDTO(current = "main", locals = listOf("main", "feat/x"))
        startAndConnect { request ->
            if (request.method == MuxyMethod.VCS_LIST_BRANCHES) {
                server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(id = request.id, result = MuxyResult.VCSBranches(expected)),
                    ),
                )
            }
        }
        assertEquals(expected, client.listBranches(projectID))
    }

    @Test
    fun `createPullRequest returns PR result`() = runBlocking {
        val expected = VCSCreatePRResultDTO(url = "https://example/pr/1", number = 1)
        startAndConnect { request ->
            if (request.method == MuxyMethod.VCS_CREATE_PR) {
                server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(id = request.id, result = MuxyResult.VCSPRCreated(expected)),
                    ),
                )
            }
        }
        val result = client.createPullRequest(
            projectID = projectID,
            title = "feat",
            body = "",
            baseBranch = "main",
            draft = false,
        )
        assertEquals(expected, result)
    }

    @Test
    fun `addWorktree refreshes worktrees on success`() = runBlocking {
        val worktreeID = UUID.randomUUID()
        val createdAt = Instant.parse("2026-05-01T10:00:00Z")
        startAndConnect { request ->
            when (request.method) {
                MuxyMethod.VCS_ADD_WORKTREE -> server.broadcast(
                    MuxyMessage.Response(MuxyResponse(id = request.id, result = MuxyResult.Ok)),
                )
                MuxyMethod.LIST_WORKTREES -> server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(
                            id = request.id,
                            result = MuxyResult.Worktrees(
                                listOf(
                                    WorktreeDTO(
                                        id = worktreeID,
                                        name = "feature",
                                        path = "/x",
                                        branch = "feat",
                                        isPrimary = false,
                                        createdAt = createdAt,
                                    ),
                                ),
                            ),
                        ),
                    ),
                )
                else -> Unit
            }
        }
        client.addWorktree(projectID, name = "feature", branch = "feat", createBranch = true)
        val list = client.projectWorktrees.value[projectID]
        assertNotNull(list)
        assertEquals(1, list!!.size)
        assertEquals(worktreeID, list.first().id)
    }

    @Test
    fun `selectProject sets activeProjectID and refreshes workspace`() = runBlocking {
        val workspaceProjectID = projectID
        val workspaceWorktreeID = UUID.randomUUID()
        val areaID = UUID.randomUUID()
        startAndConnect { request ->
            when (request.method) {
                MuxyMethod.SELECT_PROJECT -> server.broadcast(
                    MuxyMessage.Response(MuxyResponse(id = request.id, result = MuxyResult.Ok)),
                )
                MuxyMethod.GET_WORKSPACE -> server.broadcast(
                    MuxyMessage.Response(
                        MuxyResponse(
                            id = request.id,
                            result = MuxyResult.Workspace(
                                com.muxy.protocol.dto.WorkspaceDTO(
                                    projectID = workspaceProjectID,
                                    worktreeID = workspaceWorktreeID,
                                    focusedAreaID = areaID,
                                    root = com.muxy.protocol.dto.SplitNodeDTO.TabArea(
                                        com.muxy.protocol.dto.TabAreaDTO(
                                            id = areaID,
                                            projectPath = "/x",
                                            tabs = emptyList(),
                                            activeTabID = null,
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                )
                else -> Unit
            }
        }
        val ok = client.selectProject(projectID)
        assertTrue(ok)
        assertEquals(projectID, client.activeProjectID.value)
        assertEquals(workspaceWorktreeID, client.workspace.value!!.worktreeID)
    }
}

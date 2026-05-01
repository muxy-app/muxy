package com.muxy.net

import com.muxy.protocol.dto.GetVCSStatusParams
import com.muxy.protocol.dto.VCSAddWorktreeParams
import com.muxy.protocol.dto.VCSBranchesDTO
import com.muxy.protocol.dto.VCSCommitParams
import com.muxy.protocol.dto.VCSCreateBranchParams
import com.muxy.protocol.dto.VCSCreatePRParams
import com.muxy.protocol.dto.VCSCreatePRResultDTO
import com.muxy.protocol.dto.VCSDiscardFilesParams
import com.muxy.protocol.dto.VCSListBranchesParams
import com.muxy.protocol.dto.VCSPullParams
import com.muxy.protocol.dto.VCSPushParams
import com.muxy.protocol.dto.VCSRemoveWorktreeParams
import com.muxy.protocol.dto.VCSStageFilesParams
import com.muxy.protocol.dto.VCSStatusDTO
import com.muxy.protocol.dto.VCSSwitchBranchParams
import com.muxy.protocol.dto.VCSUnstageFilesParams
import com.muxy.protocol.envelope.MuxyMethod
import com.muxy.protocol.envelope.MuxyParams
import com.muxy.protocol.envelope.MuxyResponse
import com.muxy.protocol.envelope.MuxyResult
import java.util.UUID

sealed class VCSClientError : Exception() {
    data object Timeout : VCSClientError() {
        override val message: String = "The request timed out."
        private fun readResolve(): Any = Timeout
    }

    data class Server(override val message: String) : VCSClientError()

    data object UnexpectedResponse : VCSClientError() {
        override val message: String = "Unexpected response from Mac."
        private fun readResolve(): Any = UnexpectedResponse
    }
}

suspend fun MuxyClient.fetchVCSStatus(projectID: UUID): VCSStatusDTO? {
    val response = send(
        MuxyMethod.GET_VCS_STATUS,
        MuxyParams.GetVCSStatus(GetVCSStatusParams(projectID = projectID)),
    ) ?: return null
    if (response.error != null) return null
    return (response.result as? MuxyResult.VCSStatus)?.value
}

suspend fun MuxyClient.stageFiles(projectID: UUID, paths: List<String>) {
    sendThrowingVCS(
        MuxyMethod.VCS_STAGE_FILES,
        MuxyParams.VCSStageFiles(VCSStageFilesParams(projectID = projectID, paths = paths)),
    )
}

suspend fun MuxyClient.unstageFiles(projectID: UUID, paths: List<String>) {
    sendThrowingVCS(
        MuxyMethod.VCS_UNSTAGE_FILES,
        MuxyParams.VCSUnstageFiles(VCSUnstageFilesParams(projectID = projectID, paths = paths)),
    )
}

suspend fun MuxyClient.discardFiles(
    projectID: UUID,
    paths: List<String>,
    untrackedPaths: List<String>,
) {
    sendThrowingVCS(
        MuxyMethod.VCS_DISCARD_FILES,
        MuxyParams.VCSDiscardFiles(
            VCSDiscardFilesParams(projectID = projectID, paths = paths, untrackedPaths = untrackedPaths),
        ),
    )
}

suspend fun MuxyClient.vcsCommit(projectID: UUID, message: String, stageAll: Boolean) {
    sendThrowingVCS(
        MuxyMethod.VCS_COMMIT,
        MuxyParams.VCSCommit(VCSCommitParams(projectID = projectID, message = message, stageAll = stageAll)),
    )
}

suspend fun MuxyClient.vcsPush(projectID: UUID) {
    sendThrowingVCS(
        MuxyMethod.VCS_PUSH,
        MuxyParams.VCSPush(VCSPushParams(projectID = projectID)),
    )
}

suspend fun MuxyClient.vcsPull(projectID: UUID) {
    sendThrowingVCS(
        MuxyMethod.VCS_PULL,
        MuxyParams.VCSPull(VCSPullParams(projectID = projectID)),
    )
}

suspend fun MuxyClient.listBranches(projectID: UUID): VCSBranchesDTO {
    val response = send(
        MuxyMethod.VCS_LIST_BRANCHES,
        MuxyParams.VCSListBranches(VCSListBranchesParams(projectID = projectID)),
    ) ?: throw VCSClientError.Timeout
    val error = response.error
    if (error != null) throw VCSClientError.Server(error.message)
    return (response.result as? MuxyResult.VCSBranches)?.value ?: throw VCSClientError.UnexpectedResponse
}

suspend fun MuxyClient.switchBranch(projectID: UUID, branch: String) {
    sendThrowingVCS(
        MuxyMethod.VCS_SWITCH_BRANCH,
        MuxyParams.VCSSwitchBranch(VCSSwitchBranchParams(projectID = projectID, branch = branch)),
    )
}

suspend fun MuxyClient.createBranch(projectID: UUID, name: String) {
    sendThrowingVCS(
        MuxyMethod.VCS_CREATE_BRANCH,
        MuxyParams.VCSCreateBranch(VCSCreateBranchParams(projectID = projectID, name = name)),
    )
}

suspend fun MuxyClient.createPullRequest(
    projectID: UUID,
    title: String,
    body: String,
    baseBranch: String?,
    draft: Boolean,
): VCSCreatePRResultDTO {
    val response = send(
        MuxyMethod.VCS_CREATE_PR,
        MuxyParams.VCSCreatePR(
            VCSCreatePRParams(
                projectID = projectID,
                title = title,
                body = body,
                baseBranch = baseBranch,
                draft = draft,
            ),
        ),
    ) ?: throw VCSClientError.Timeout
    val error = response.error
    if (error != null) throw VCSClientError.Server(error.message)
    return (response.result as? MuxyResult.VCSPRCreated)?.value ?: throw VCSClientError.UnexpectedResponse
}

suspend fun MuxyClient.addWorktree(
    projectID: UUID,
    name: String,
    branch: String,
    createBranch: Boolean,
) {
    sendThrowingVCS(
        MuxyMethod.VCS_ADD_WORKTREE,
        MuxyParams.VCSAddWorktree(
            VCSAddWorktreeParams(
                projectID = projectID,
                name = name,
                branch = branch,
                createBranch = createBranch,
            ),
        ),
    )
    refreshWorktrees(projectID)
}

suspend fun MuxyClient.removeWorktree(projectID: UUID, worktreeID: UUID) {
    sendThrowingVCS(
        MuxyMethod.VCS_REMOVE_WORKTREE,
        MuxyParams.VCSRemoveWorktree(VCSRemoveWorktreeParams(projectID = projectID, worktreeID = worktreeID)),
    )
    refreshWorktrees(projectID)
}

private suspend fun MuxyClient.sendThrowingVCS(
    method: MuxyMethod,
    params: MuxyParams,
): MuxyResponse {
    val response = send(method, params) ?: throw VCSClientError.Timeout
    val error = response.error
    if (error != null) throw VCSClientError.Server(error.message)
    return response
}

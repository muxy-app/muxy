import Foundation
import MuxyShared

extension ConnectionManager {
    func fetchVCSStatus(projectID: UUID) async -> VCSStatusDTO? {
        let params = GetVCSStatusParams(projectID: projectID)
        guard let response = await send(.getVCSStatus, params: .getVCSStatus(params)) else {
            return nil
        }
        if case let .vcsStatus(status) = response.result {
            return status
        }
        return nil
    }

    func stageFiles(projectID: UUID, paths: [String]) async throws {
        let params = VCSStageFilesParams(projectID: projectID, paths: paths)
        try await sendThrowing(.vcsStageFiles, params: .vcsStageFiles(params))
    }

    func unstageFiles(projectID: UUID, paths: [String]) async throws {
        let params = VCSUnstageFilesParams(projectID: projectID, paths: paths)
        try await sendThrowing(.vcsUnstageFiles, params: .vcsUnstageFiles(params))
    }

    func discardFiles(projectID: UUID, paths: [String], untrackedPaths: [String]) async throws {
        let params = VCSDiscardFilesParams(
            projectID: projectID,
            paths: paths,
            untrackedPaths: untrackedPaths
        )
        try await sendThrowing(.vcsDiscardFiles, params: .vcsDiscardFiles(params))
    }

    func vcsCommit(projectID: UUID, message: String, stageAll: Bool) async throws {
        let params = VCSCommitParams(projectID: projectID, message: message, stageAll: stageAll)
        try await sendThrowing(.vcsCommit, params: .vcsCommit(params), timeout: .seconds(60))
    }

    func vcsPush(projectID: UUID) async throws {
        let params = VCSPushParams(projectID: projectID)
        try await sendThrowing(.vcsPush, params: .vcsPush(params), timeout: .seconds(120))
    }

    func vcsPull(projectID: UUID) async throws {
        let params = VCSPullParams(projectID: projectID)
        try await sendThrowing(.vcsPull, params: .vcsPull(params), timeout: .seconds(120))
    }

    func listBranches(projectID: UUID) async throws -> VCSBranchesDTO {
        let params = VCSListBranchesParams(projectID: projectID)
        guard let response = await send(.vcsListBranches, params: .vcsListBranches(params)) else {
            throw VCSClientError.timeout
        }
        if let error = response.error {
            throw VCSClientError.server(error.message)
        }
        if case let .vcsBranches(branches) = response.result {
            return branches
        }
        throw VCSClientError.unexpectedResponse
    }

    func switchBranch(projectID: UUID, branch: String) async throws {
        let params = VCSSwitchBranchParams(projectID: projectID, branch: branch)
        try await sendThrowing(.vcsSwitchBranch, params: .vcsSwitchBranch(params), timeout: .seconds(30))
    }

    func createBranch(projectID: UUID, name: String) async throws {
        let params = VCSCreateBranchParams(projectID: projectID, name: name)
        try await sendThrowing(.vcsCreateBranch, params: .vcsCreateBranch(params), timeout: .seconds(30))
    }

    func createPullRequest(
        projectID: UUID,
        title: String,
        body: String,
        baseBranch: String?,
        draft: Bool
    ) async throws -> VCSCreatePRResultDTO {
        let params = VCSCreatePRParams(
            projectID: projectID,
            title: title,
            body: body,
            baseBranch: baseBranch,
            draft: draft
        )
        guard let response = await send(.vcsCreatePR, params: .vcsCreatePR(params), timeout: .seconds(120)) else {
            throw VCSClientError.timeout
        }
        if let error = response.error {
            throw VCSClientError.server(error.message)
        }
        if case let .vcsPRCreated(info) = response.result {
            return info
        }
        throw VCSClientError.unexpectedResponse
    }

    func addWorktree(
        projectID: UUID,
        name: String,
        branch: String,
        createBranch: Bool
    ) async throws {
        let params = VCSAddWorktreeParams(
            projectID: projectID,
            name: name,
            branch: branch,
            createBranch: createBranch
        )
        try await sendThrowing(.vcsAddWorktree, params: .vcsAddWorktree(params), timeout: .seconds(60))
        await refreshWorktrees(projectID: projectID)
    }

    func removeWorktree(projectID: UUID, worktreeID: UUID) async throws {
        let params = VCSRemoveWorktreeParams(projectID: projectID, worktreeID: worktreeID)
        try await sendThrowing(.vcsRemoveWorktree, params: .vcsRemoveWorktree(params), timeout: .seconds(60))
        await refreshWorktrees(projectID: projectID)
    }

    func selectWorktree(projectID: UUID, worktreeID: UUID) async throws {
        let params = SelectWorktreeParams(projectID: projectID, worktreeID: worktreeID)
        try await sendThrowing(.selectWorktree, params: .selectWorktree(params))
        await refreshWorkspace(projectID: projectID)
    }

    private func sendThrowing(
        _ method: MuxyMethod,
        params: MuxyParams,
        timeout: Duration = .seconds(30)
    ) async throws {
        guard let response = await send(method, params: params, timeout: timeout) else {
            throw VCSClientError.timeout
        }
        if let error = response.error {
            throw VCSClientError.server(error.message)
        }
    }
}

enum VCSClientError: LocalizedError {
    case timeout
    case server(String)
    case unexpectedResponse

    var errorDescription: String? {
        switch self {
        case .timeout: "The request timed out."
        case let .server(message): message
        case .unexpectedResponse: "Unexpected response from Mac."
        }
    }
}

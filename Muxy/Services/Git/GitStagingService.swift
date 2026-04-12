import Foundation

struct GitStagingService {
    func stageFiles(repoPath: String, paths: [String]) async throws {
        for path in paths {
            try GitValidation.validatePath(repoPath: repoPath, relativePath: path)
        }
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["add", "--"] + paths)
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to stage files." : result.stderr)
        }
    }

    func stageAll(repoPath: String) async throws {
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["add", "-A"])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to stage all files." : result.stderr)
        }
    }

    func unstageFiles(repoPath: String, paths: [String]) async throws {
        for path in paths {
            try GitValidation.validatePath(repoPath: repoPath, relativePath: path)
        }
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["reset", "HEAD", "--"] + paths)
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to unstage files." : result.stderr)
        }
    }

    func unstageAll(repoPath: String) async throws {
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["reset", "HEAD"])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to unstage all files." : result.stderr)
        }
    }

    func discardFiles(repoPath: String, paths: [String], untrackedPaths: [String]) async throws {
        for path in paths + untrackedPaths {
            try GitValidation.validatePath(repoPath: repoPath, relativePath: path)
        }

        if !paths.isEmpty {
            let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["checkout", "--"] + paths)
            guard result.status == 0 else {
                throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to discard changes." : result.stderr)
            }
        }

        for relativePath in untrackedPaths {
            let fullPath = (repoPath as NSString).appendingPathComponent(relativePath)
            try FileManager.default.removeItem(atPath: fullPath)
        }
    }

    func discardAll(repoPath: String) async throws {
        let checkoutResult = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["checkout", "--", "."])
        guard checkoutResult.status == 0 else {
            throw GitError.commandFailed(
                checkoutResult.stderr.isEmpty ? "Failed to discard tracked changes." : checkoutResult.stderr
            )
        }

        let cleanResult = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["clean", "-fd"])
        guard cleanResult.status == 0 else {
            throw GitError.commandFailed(
                cleanResult.stderr.isEmpty ? "Failed to clean untracked files." : cleanResult.stderr
            )
        }
    }
}

import Foundation

struct GitBranchService {
    private static let allowedBranchCharacters = CharacterSet.alphanumerics
        .union(CharacterSet(charactersIn: "._/-"))

    func currentBranch(repoPath: String) async throws -> String {
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["rev-parse", "--abbrev-ref", "HEAD"]
        )
        guard result.status == 0 else {
            throw GitError.commandFailed("Failed to get current branch.")
        }
        return result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    func headSha(repoPath: String) async -> String? {
        let result = try? await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["rev-parse", "HEAD"]
        )
        guard let result, result.status == 0 else { return nil }
        let trimmed = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    func aheadBehind(repoPath: String, branch: String) async -> GitAheadBehind {
        async let upstreamTask = GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["rev-parse", "--abbrev-ref", "\(branch)@{upstream}"]
        )
        async let countsTask = GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["rev-list", "--left-right", "--count", "\(branch)...\(branch)@{upstream}"]
        )

        let upstreamResult = try? await upstreamTask
        guard let upstreamResult, upstreamResult.status == 0 else {
            _ = try? await countsTask
            return GitAheadBehind(ahead: 0, behind: 0, hasUpstream: false)
        }

        guard let countsResult = try? await countsTask, countsResult.status == 0 else {
            return GitAheadBehind(ahead: 0, behind: 0, hasUpstream: true)
        }
        let parts = countsResult.stdout
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .split(whereSeparator: { $0 == "\t" || $0 == " " })
        guard parts.count == 2,
              let ahead = Int(parts[0]),
              let behind = Int(parts[1])
        else {
            return GitAheadBehind(ahead: 0, behind: 0, hasUpstream: true)
        }
        return GitAheadBehind(ahead: ahead, behind: behind, hasUpstream: true)
    }

    func hasRemoteBranch(repoPath: String, branch: String) async -> Bool {
        let result = try? await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["ls-remote", "--heads", "origin", branch]
        )
        guard let result, result.status == 0 else { return false }
        return !result.stdout.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    func listRemoteBranches(repoPath: String) async throws -> [String] {
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["ls-remote", "--heads", "origin"]
        )
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to list remote branches." : result.stderr)
        }
        let prefix = "refs/heads/"
        return result.stdout
            .split(separator: "\n", omittingEmptySubsequences: true)
            .compactMap { line -> String? in
                let parts = line.split(separator: "\t", maxSplits: 1, omittingEmptySubsequences: true)
                guard parts.count == 2 else { return nil }
                let ref = parts[1].trimmingCharacters(in: .whitespaces)
                guard ref.hasPrefix(prefix) else { return nil }
                return String(ref.dropFirst(prefix.count))
            }
            .sorted { $0.localizedStandardCompare($1) == .orderedAscending }
    }

    func defaultBranch(repoPath: String) async -> String? {
        if let cached = GitMetadataCache.shared.cachedDefaultBranch(repoPath: repoPath) {
            return cached
        }
        let resolved = await resolveDefaultBranch(repoPath: repoPath)
        if resolved != nil {
            GitMetadataCache.shared.storeDefaultBranch(resolved, repoPath: repoPath)
        }
        return resolved
    }

    func listBranches(repoPath: String) async throws -> [String] {
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["branch", "--list", "--format=%(refname:short)"]
        )
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to list branches." : result.stderr)
        }
        return result.stdout
            .split(separator: "\n", omittingEmptySubsequences: true)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .sorted { $0.localizedStandardCompare($1) == .orderedAscending }
    }

    func switchBranch(repoPath: String, branch: String) async throws {
        guard !branch.isEmpty,
              branch.unicodeScalars.allSatisfy({ Self.allowedBranchCharacters.contains($0) })
        else {
            throw GitError.commandFailed("Invalid branch name.")
        }
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["switch", branch])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to switch branch." : result.stderr)
        }
    }

    func createAndSwitchBranch(repoPath: String, name: String) async throws {
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedName.isEmpty,
              !trimmedName.hasPrefix("-"),
              trimmedName.unicodeScalars.allSatisfy({ Self.allowedBranchCharacters.contains($0) })
        else {
            throw GitError.commandFailed("Invalid branch name.")
        }
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["switch", "-c", trimmedName])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to create branch." : result.stderr)
        }
    }

    func createBranch(repoPath: String, name: String, startPoint: String) async throws {
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedName.isEmpty,
              !trimmedName.hasPrefix("-"),
              trimmedName.unicodeScalars.allSatisfy({ Self.allowedBranchCharacters.contains($0) })
        else {
            throw GitError.commandFailed("Invalid branch name.")
        }
        try GitValidation.validateHash(startPoint)
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["branch", "--", trimmedName, startPoint])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to create branch." : result.stderr)
        }
    }

    private func resolveDefaultBranch(repoPath: String) async -> String? {
        let symbolic = try? await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]
        )
        if let symbolic, symbolic.status == 0 {
            let value = symbolic.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
            if value.hasPrefix("origin/") {
                return String(value.dropFirst("origin/".count))
            }
            if !value.isEmpty { return value }
        }

        if let ghPath = GitProcessRunner.resolveExecutable("gh") {
            let result = try? await GitProcessRunner.runCommand(
                executable: ghPath,
                arguments: ["repo", "view", "--json", "defaultBranchRef", "-q", ".defaultBranchRef.name"],
                workingDirectory: repoPath
            )
            if let result, result.status == 0 {
                let trimmed = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
                if !trimmed.isEmpty { return trimmed }
            }
        }

        return nil
    }
}

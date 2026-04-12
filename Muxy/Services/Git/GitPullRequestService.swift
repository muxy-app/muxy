import Foundation

struct GitPullRequestService {
    func isGhInstalled() async -> Bool {
        if let cached = GitMetadataCache.shared.cachedGhInstalled() {
            return cached
        }
        let installed = GitProcessRunner.resolveExecutable("gh") != nil
        GitMetadataCache.shared.storeGhInstalled(installed)
        return installed
    }

    func cachedPullRequestInfo(
        repoPath: String,
        branch: String,
        headSha: String,
        forceFresh: Bool
    ) async -> PRInfo? {
        if !forceFresh, let cached = GitMetadataCache.shared.cachedPRInfo(
            repoPath: repoPath,
            branch: branch,
            headSha: headSha
        ) {
            return cached
        }
        let info = await pullRequestInfo(repoPath: repoPath, branch: branch)
        GitMetadataCache.shared.storePRInfo(info, repoPath: repoPath, branch: branch, headSha: headSha)
        return info
    }

    func pullRequestInfo(repoPath: String, branch: String) async -> PRInfo? {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else { return nil }
        let result = try? await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: [
                "pr", "view", branch,
                "--json", "url,number,state,isDraft,baseRefName,mergeable,statusCheckRollup",
            ],
            workingDirectory: repoPath
        )
        guard let result, result.status == 0,
              let data = result.stdout.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }

        guard let url = json["url"] as? String,
              let number = json["number"] as? Int,
              let stateRaw = json["state"] as? String
        else { return nil }

        let state = PRState(rawValue: stateRaw) ?? .open
        let isDraft = json["isDraft"] as? Bool ?? false
        let baseBranch = json["baseRefName"] as? String ?? ""
        let mergeableRaw = json["mergeable"] as? String
        let mergeable: Bool? = switch mergeableRaw {
        case "MERGEABLE": true
        case "CONFLICTING": false
        default: nil
        }

        let rollup = json["statusCheckRollup"] as? [[String: Any]] ?? []
        let checks = Self.parseStatusChecks(rollup)

        return PRInfo(
            url: url,
            number: number,
            state: state,
            isDraft: isDraft,
            baseBranch: baseBranch,
            mergeable: mergeable,
            checks: checks
        )
    }

    func createPullRequest(
        repoPath: String,
        branch: String,
        baseBranch: String,
        title: String,
        body: String,
        draft: Bool = false
    ) async throws -> PRInfo {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else {
            throw PRCreateError.ghNotInstalled
        }

        var arguments: [String] = [
            "pr", "create",
            "--head", branch,
            "--base", baseBranch,
            "--title", title,
        ]
        arguments.append("--body")
        arguments.append(body)
        if draft {
            arguments.append("--draft")
        }

        let createResult = try await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: arguments,
            workingDirectory: repoPath
        )
        guard createResult.status == 0 else {
            let message = createResult.stderr.isEmpty ? createResult.stdout : createResult.stderr
            throw PRCreateError.commandFailed(
                message.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    ? "Failed to create pull request."
                    : message.trimmingCharacters(in: .whitespacesAndNewlines)
            )
        }

        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath, branch: branch)

        if let info = await pullRequestInfo(repoPath: repoPath, branch: branch) {
            return info
        }

        let fallbackURL = createResult.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        throw PRCreateError.commandFailed(
            fallbackURL.isEmpty
                ? "Pull request created but could not be read back."
                : "Pull request created at \(fallbackURL) but could not be read back."
        )
    }

    func mergePullRequest(
        repoPath: String,
        number: Int,
        method: PRMergeMethod = .merge,
        deleteBranch: Bool = true
    ) async throws {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else {
            throw PRCreateError.ghNotInstalled
        }
        var arguments = ["pr", "merge", String(number), method.ghFlag]
        if deleteBranch {
            arguments.append("--delete-branch")
        }
        let result = try await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: arguments,
            workingDirectory: repoPath
        )
        guard result.status == 0 else {
            let message = result.stderr.isEmpty ? result.stdout : result.stderr
            throw PRCreateError.commandFailed(
                message.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    ? "Failed to merge pull request."
                    : message.trimmingCharacters(in: .whitespacesAndNewlines)
            )
        }
        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath)
    }

    func closePullRequest(repoPath: String, number: Int) async throws {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else {
            throw PRCreateError.ghNotInstalled
        }
        let result = try await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: ["pr", "close", String(number)],
            workingDirectory: repoPath
        )
        guard result.status == 0 else {
            let message = result.stderr.isEmpty ? result.stdout : result.stderr
            throw PRCreateError.commandFailed(
                message.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    ? "Failed to close pull request."
                    : message.trimmingCharacters(in: .whitespacesAndNewlines)
            )
        }
        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath)
    }

    func deleteRemoteBranch(repoPath: String, branch: String, remote: String = "origin") async throws {
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["push", remote, "--delete", branch]
        )
        guard result.status == 0 else {
            throw GitError.commandFailed(
                result.stderr.isEmpty ? "Failed to delete remote branch \(branch)." : result.stderr
            )
        }
    }

    private static func parseStatusChecks(_ rollup: [[String: Any]]) -> PRChecks {
        if rollup.isEmpty {
            return PRChecks(status: .none, passing: 0, failing: 0, pending: 0, total: 0)
        }

        var passing = 0
        var failing = 0
        var pending = 0

        for entry in rollup {
            let typename = entry["__typename"] as? String ?? ""
            let outcome: String
            if typename == "CheckRun" {
                let status = (entry["status"] as? String ?? "").uppercased()
                let conclusion = (entry["conclusion"] as? String ?? "").uppercased()
                if status != "COMPLETED" {
                    outcome = "PENDING"
                } else {
                    outcome = conclusion
                }
            } else {
                outcome = (entry["state"] as? String ?? "").uppercased()
            }

            switch outcome {
            case "SUCCESS",
                 "NEUTRAL",
                 "SKIPPED":
                passing += 1
            case "FAILURE",
                 "ERROR",
                 "CANCELLED",
                 "TIMED_OUT",
                 "ACTION_REQUIRED",
                 "STARTUP_FAILURE":
                failing += 1
            case "PENDING",
                 "QUEUED",
                 "IN_PROGRESS",
                 "WAITING",
                 "REQUESTED",
                 "EXPECTED":
                pending += 1
            default:
                pending += 1
            }
        }

        let total = passing + failing + pending
        let status: PRChecksStatus = if failing > 0 {
            .failure
        } else if pending > 0 {
            .pending
        } else if passing > 0 {
            .success
        } else {
            .none
        }
        return PRChecks(status: status, passing: passing, failing: failing, pending: pending, total: total)
    }
}

import Foundation

struct GitCommitService {
    private static let commitFieldSeparator = "\u{1F}"
    private static let commitRecordSeparator = "\u{1E}"

    private static let logFormat = [
        "%H", "%h", "%s", "%an", "%aI", "%D", "%P",
    ].joined(separator: commitFieldSeparator) + commitRecordSeparator

    private static let allowedTagCharacters = CharacterSet.alphanumerics
        .union(CharacterSet(charactersIn: "._/-"))

    func commit(repoPath: String, message: String) async throws -> String {
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["commit", "-m", message])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to commit." : result.stderr)
        }

        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath)

        let hashResult = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["rev-parse", "--short", "HEAD"])
        guard hashResult.status == 0 else { return "" }
        return hashResult.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    func push(repoPath: String) async throws {
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["push"])
        guard result.status == 0 else {
            if result.stderr.contains("has no upstream branch") {
                throw GitError.noUpstreamBranch
            }
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to push." : result.stderr)
        }
        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath)
    }

    func pushSetUpstream(repoPath: String, branch: String) async throws {
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["push", "--set-upstream", "origin", branch])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to push." : result.stderr)
        }
        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath, branch: branch)
    }

    func pull(repoPath: String) async throws {
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["pull"])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to pull." : result.stderr)
        }
        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath)
    }

    func commitLog(repoPath: String, maxCount: Int = 100, skip: Int = 0) async throws -> [GitCommit] {
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: [
                "log",
                "--decorate=full",
                "--format=\(Self.logFormat)",
                "--max-count=\(maxCount)",
                "--skip=\(skip)",
            ]
        )
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to load commit history." : result.stderr)
        }
        return Self.parseCommitLog(result.stdout)
    }

    func cherryPick(repoPath: String, hash: String) async throws {
        try GitValidation.validateHash(hash)
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["cherry-pick", hash])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to cherry-pick." : result.stderr)
        }
    }

    func createTag(repoPath: String, name: String, hash: String) async throws {
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedName.isEmpty,
              !trimmedName.hasPrefix("-"),
              trimmedName.unicodeScalars.allSatisfy({ Self.allowedTagCharacters.contains($0) })
        else {
            throw GitError.commandFailed("Invalid tag name.")
        }
        try GitValidation.validateHash(hash)
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["tag", "--", trimmedName, hash])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to create tag." : result.stderr)
        }
    }

    func checkoutDetached(repoPath: String, hash: String) async throws {
        try GitValidation.validateHash(hash)
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["checkout", "--detach", hash])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to checkout." : result.stderr)
        }
    }

    private static func parseCommitLog(_ raw: String) -> [GitCommit] {
        let records = raw.split(separator: Character(commitRecordSeparator), omittingEmptySubsequences: true)
        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime]

        return records.compactMap { record in
            let fields = record.trimmingCharacters(in: .whitespacesAndNewlines)
                .split(separator: Character(commitFieldSeparator), maxSplits: 6, omittingEmptySubsequences: false)
            guard fields.count >= 7 else { return nil }

            let hash = String(fields[0])
            let shortHash = String(fields[1])
            let subject = String(fields[2])
            let authorName = String(fields[3])
            let dateString = String(fields[4])
            let refsRaw = String(fields[5])
            let parentsRaw = String(fields[6])

            let date = dateFormatter.date(from: dateString) ?? Date.distantPast
            let refs = Self.parseRefs(refsRaw)
            let parents = parentsRaw.split(separator: " ").map(String.init)

            return GitCommit(
                hash: hash,
                shortHash: shortHash,
                subject: subject,
                authorName: authorName,
                authorDate: date,
                refs: refs,
                parentHashes: parents
            )
        }
    }

    private static func parseRefs(_ raw: String) -> [GitRef] {
        guard !raw.isEmpty else { return [] }
        return raw.split(separator: ",").compactMap { segment in
            let trimmed = segment.trimmingCharacters(in: .whitespaces)
            if trimmed == "HEAD" {
                return GitRef(name: "HEAD", kind: .head)
            }
            if trimmed.hasPrefix("HEAD -> ") {
                let branch = String(trimmed.dropFirst("HEAD -> ".count))
                    .replacingOccurrences(of: "refs/heads/", with: "")
                return GitRef(name: branch, kind: .localBranch)
            }
            if trimmed.hasPrefix("tag: ") {
                let tag = String(trimmed.dropFirst("tag: ".count))
                    .replacingOccurrences(of: "refs/tags/", with: "")
                return GitRef(name: tag, kind: .tag)
            }
            if trimmed.hasPrefix("refs/heads/") {
                let name = String(trimmed.dropFirst("refs/heads/".count))
                return GitRef(name: name, kind: .localBranch)
            }
            if trimmed.hasPrefix("refs/remotes/") {
                let name = String(trimmed.dropFirst("refs/remotes/".count))
                return GitRef(name: name, kind: .remoteBranch)
            }
            if trimmed.hasPrefix("refs/tags/") {
                let name = String(trimmed.dropFirst("refs/tags/".count))
                return GitRef(name: name, kind: .tag)
            }
            return GitRef(name: trimmed, kind: .localBranch)
        }
    }
}

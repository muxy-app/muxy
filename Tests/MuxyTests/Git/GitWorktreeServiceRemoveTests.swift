import Foundation
import Testing

@testable import Muxy

@Suite("GitWorktreeService.removeWorktree")
struct GitWorktreeServiceRemoveTests {
    @Test("removes a normal worktree")
    func removesNormalWorktree() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1", message: "base")
        let worktreePath = repo.siblingPath("feature-wt")
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repo.path,
            path: worktreePath,
            branch: "feature",
            createBranch: true,
            baseBranch: nil
        )

        try await GitWorktreeService.shared.removeWorktree(repoPath: repo.path, path: worktreePath, force: true)

        let records = try await GitWorktreeService.shared.listWorktrees(repoPath: repo.path)
        #expect(!records.contains { $0.path == worktreePath })
    }

    @Test("succeeds when the worktree folder is gone and git admin metadata is orphaned")
    func succeedsForOrphanedWorktree() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1", message: "base")
        let worktreePath = repo.siblingPath("orphan-wt")
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repo.path,
            path: worktreePath,
            branch: "feature",
            createBranch: true,
            baseBranch: nil
        )
        try FileManager.default.removeItem(atPath: worktreePath)
        try repo.orphanWorktreeAdmin(named: "orphan-wt")

        try await GitWorktreeService.shared.removeWorktree(repoPath: repo.path, path: worktreePath, force: true)

        let records = try await GitWorktreeService.shared.listWorktrees(repoPath: repo.path)
        #expect(!records.contains { $0.path == worktreePath })
    }

    @Test("throws when git leaves the worktree registered")
    func throwsWhenWorktreeSurvivesRemoval() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1", message: "base")
        let worktreePath = repo.siblingPath("locked-wt")
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repo.path,
            path: worktreePath,
            branch: "feature",
            createBranch: true,
            baseBranch: nil
        )
        try repo.run("worktree", "lock", worktreePath)

        await #expect(throws: Error.self) {
            try await GitWorktreeService.shared.removeWorktree(repoPath: repo.path, path: worktreePath, force: false)
        }
        let target = GitWorktreeService.canonicalPath(worktreePath)
        let records = try await GitWorktreeService.shared.listWorktrees(repoPath: repo.path)
        #expect(records.contains { GitWorktreeService.canonicalPath($0.path) == target })
    }

    @Test("rejects a directory that is not a registered worktree")
    func rejectsUnregisteredDirectory() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        let directory = repo.siblingPath("not-a-worktree")
        try FileManager.default.createDirectory(atPath: directory, withIntermediateDirectories: true)

        await #expect(throws: Error.self) {
            try await GitWorktreeService.shared.removeWorktree(
                repoPath: repo.path,
                path: directory,
                force: true
            )
        }

        #expect(FileManager.default.fileExists(atPath: directory))
    }

    @Test("trusted tracked cleanup preserves a deregistered residual without teardown")
    func trackedResidualDirectoryIsReportedWithoutTeardown() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        let directory = repo.siblingPath("tracked-residual")
        try FileManager.default.createDirectory(atPath: directory, withIntermediateDirectories: true)
        let configDirectory = URL(fileURLWithPath: repo.path).appendingPathComponent(".muxy", isDirectory: true)
        try FileManager.default.createDirectory(at: configDirectory, withIntermediateDirectories: true)
        let config = WorktreeConfig(
            setup: [],
            teardown: [WorktreeConfig.SetupCommand(command: "touch teardown-ran")]
        )
        try JSONEncoder().encode(config).write(to: configDirectory.appendingPathComponent("worktree.json"))
        let worktree = Worktree(name: "Tracked residual", path: directory, isPrimary: false)

        let dirRemoved = try await WorktreeStore.cleanupOnDisk(worktree: worktree, repoPath: repo.path)

        #expect(dirRemoved == false)
        #expect(FileManager.default.fileExists(atPath: directory))
        #expect(!FileManager.default.fileExists(atPath: URL(fileURLWithPath: directory)
            .appendingPathComponent("teardown-ran").path))
    }

    @Test("tracked cleanup verifies the resolved path instead of its stored remote form")
    func trackedCleanupUsesResolvedPath() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        let directory = repo.siblingPath("resolved-residual")
        try FileManager.default.createDirectory(atPath: directory, withIntermediateDirectories: true)
        let storedPath = "~/muxy-remote-\(UUID().uuidString)"
        let worktree = Worktree(name: "Remote residual", path: storedPath, isPrimary: false)
        let resolution = GitWorktreeService.WorktreePathResolution(
            path: directory,
            identityPaths: [directory, storedPath],
            remoteHomePath: "/home/test"
        )

        let dirRemoved = try await WorktreeStore.cleanupOnDisk(
            worktree: worktree,
            repoPath: repo.path,
            pathResolution: resolution
        )

        #expect(dirRemoved == false)
        #expect(FileManager.default.fileExists(atPath: directory))
    }

    @Test("cleanupOnDisk removes the worktree but keeps its branch")
    func cleanupKeepsBranch() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1", message: "base")
        let worktreePath = repo.siblingPath("keep-branch-wt")
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repo.path,
            path: worktreePath,
            branch: "feature",
            createBranch: true,
            baseBranch: nil
        )

        let worktree = Worktree(name: "keep-branch-wt", path: worktreePath, branch: "feature", isPrimary: false)
        let dirRemoved = try await WorktreeStore.cleanupOnDisk(worktree: worktree, repoPath: repo.path)

        let records = try await GitWorktreeService.shared.listWorktrees(repoPath: repo.path)
        #expect(!records.contains { $0.path == worktreePath })
        #expect(repo.branchExists("feature"))
        #expect(dirRemoved == true)
    }

    @Test("cleanupOnDisk without force preserves a dirty worktree")
    func cleanupWithoutForcePreservesDirtyWorktree() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1", message: "base")
        let worktreePath = repo.siblingPath("dirty-wt")
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repo.path,
            path: worktreePath,
            branch: "dirty-feature",
            createBranch: true,
            baseBranch: nil
        )
        try "dirty".write(
            toFile: URL(fileURLWithPath: worktreePath).appendingPathComponent("a.txt").path,
            atomically: true,
            encoding: .utf8
        )
        let worktree = Worktree(name: "dirty-wt", path: worktreePath, branch: "dirty-feature", isPrimary: false)

        await #expect(throws: Error.self) {
            try await WorktreeStore.cleanupOnDisk(
                worktree: worktree,
                repoPath: repo.path,
                force: false
            )
        }

        #expect(FileManager.default.fileExists(atPath: worktreePath))
        let records = try await GitWorktreeService.shared.listWorktrees(repoPath: repo.path)
        let target = GitWorktreeService.canonicalPath(worktreePath)
        #expect(records.contains { GitWorktreeService.canonicalPath($0.path) == target })
    }

    @Test("uncommitted change inspection surfaces git failures")
    func uncommittedChangeInspectionSurfacesGitFailures() async {
        await #expect(throws: Error.self) {
            try await GitWorktreeService.shared.uncommittedChanges(
                worktreePath: "/tmp/muxy-missing-worktree-\(UUID().uuidString)"
            )
        }
    }

    @Test("a timeout after git deregisters the worktree is reconciled as success")
    func reconcilesTimeoutAfterDeregistration() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1", message: "base")
        let worktreePath = repo.siblingPath("timeout-wt")
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repo.path,
            path: worktreePath,
            branch: "timeout-feature",
            createBranch: true,
            baseBranch: nil
        )

        try await GitWorktreeService.shared.removeWorktree(
            repoPath: repo.path,
            path: worktreePath,
            force: true,
            removalRunner: { repoPath, arguments, context, timeout in
                _ = try await GitProcessRunner.runGit(
                    repoPath: repoPath,
                    arguments: arguments,
                    context: context,
                    timeout: timeout
                )
                throw GitProcessError.timedOut(timeout)
            }
        )

        let records = try await GitWorktreeService.shared.listWorktrees(repoPath: repo.path)
        let target = GitWorktreeService.canonicalPath(worktreePath)
        #expect(!records.contains { GitWorktreeService.canonicalPath($0.path) == target })
    }

    @Test("a timeout cannot start reconciliation after the removal deadline")
    func doesNotReconcileAfterDeadline() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1", message: "base")
        let worktreePath = repo.siblingPath("expired-timeout-wt")
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repo.path,
            path: worktreePath,
            branch: "expired-timeout-feature",
            createBranch: true,
            baseBranch: nil
        )

        await #expect(throws: GitProcessError.self) {
            try await GitWorktreeService.shared.removeWorktree(
                repoPath: repo.path,
                path: worktreePath,
                force: true,
                timeout: 0.5,
                removalRunner: { repoPath, arguments, context, timeout in
                    _ = try await GitProcessRunner.runGit(
                        repoPath: repoPath,
                        arguments: arguments,
                        context: context,
                        timeout: timeout
                    )
                    try await Task.sleep(for: .seconds(timeout + 0.1))
                    throw GitProcessError.timedOut(timeout)
                }
            )
        }
    }

    @Test("a nonzero result after deregistration preserves and reports the residual directory")
    func reconcilesNonzeroResultAfterDeregistration() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1", message: "base")
        let worktreePath = repo.siblingPath("nonzero-wt")
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repo.path,
            path: worktreePath,
            branch: "nonzero-feature",
            createBranch: true,
            baseBranch: nil
        )
        let adminPath = URL(fileURLWithPath: repo.path)
            .appendingPathComponent(".git/worktrees/nonzero-wt", isDirectory: true)
            .path

        try await GitWorktreeService.shared.removeWorktree(
            repoPath: repo.path,
            path: worktreePath,
            force: true,
            removalRunner: { _, _, _, _ in
                try FileManager.default.removeItem(atPath: adminPath)
                return GitProcessResult(
                    status: 1,
                    stdout: "",
                    stdoutData: Data(),
                    stderr: "residual directory remains",
                    truncated: false
                )
            }
        )

        let records = try await GitWorktreeService.shared.listWorktrees(repoPath: repo.path)
        #expect(!records.contains { GitWorktreeService.canonicalPath($0.path) == worktreePath })
        #expect(FileManager.default.fileExists(atPath: worktreePath))
    }

    @Test("local relative paths resolve from the selected repository")
    func resolvesLocalRelativeWorktreePaths() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        let expected = repo.siblingPath("relative-wt")

        let resolution = try await GitWorktreeService.resolveWorktreePath(
            "../relative-wt",
            repoPath: repo.path,
            context: .local,
            timeout: 10
        )
        let resolvedExpected = try await GitWorktreeService.canonicalLocalPaths(
            [expected],
            deadline: OperationDeadline(timeout: 10)
        )[0]

        #expect(resolution.path == resolvedExpected)
    }

    @Test("local path resolution rejects an expired deadline")
    func localPathResolutionHonorsDeadline() async {
        await #expect(throws: AsyncTimeoutError.self) {
            try await GitWorktreeService.resolveWorktreePath(
                "/tmp/worktree",
                repoPath: "/tmp/repo",
                context: .local,
                deadline: OperationDeadline(timeout: 0)
            )
        }
    }

    @Test("local relative paths use the physical selected repository")
    func resolvesRelativePathsFromSymlinkedRepository() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-relative-symlink-\(UUID().uuidString)", isDirectory: true)
        let repository = root.appendingPathComponent("actual/repo", isDirectory: true)
        let aliases = root.appendingPathComponent("aliases", isDirectory: true)
        let alias = aliases.appendingPathComponent("repo-alias")
        try FileManager.default.createDirectory(at: repository, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: aliases, withIntermediateDirectories: true)
        try FileManager.default.createSymbolicLink(at: alias, withDestinationURL: repository)
        defer { try? FileManager.default.removeItem(at: root) }
        let expected = root.appendingPathComponent("actual/feature").path

        let resolution = try await GitWorktreeService.resolveWorktreePath(
            "../feature",
            repoPath: alias.path,
            context: .local,
            timeout: 10
        )
        let resolvedExpected = try await GitWorktreeService.canonicalLocalPaths(
            [expected],
            deadline: OperationDeadline(timeout: 10)
        )[0]

        #expect(resolution.path == resolvedExpected)
    }

    @Test("local path resolution follows dangling symlink targets with bounded cycles")
    func resolvesLocalDanglingSymlinkTargets() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-local-dangling-\(UUID().uuidString)", isDirectory: true)
        let physicalParent = root.appendingPathComponent("physical", isDirectory: true)
        let missingTarget = physicalParent.appendingPathComponent("missing-worktree")
        let absoluteAlias = root.appendingPathComponent("absolute-alias")
        let relativeAlias = root.appendingPathComponent("relative-alias")
        let cycleAlias = root.appendingPathComponent("cycle-alias")
        try FileManager.default.createDirectory(at: physicalParent, withIntermediateDirectories: true)
        try FileManager.default.createSymbolicLink(at: absoluteAlias, withDestinationURL: missingTarget)
        try FileManager.default.createSymbolicLink(
            atPath: relativeAlias.path,
            withDestinationPath: "physical/missing-worktree"
        )
        try FileManager.default.createSymbolicLink(
            atPath: cycleAlias.path,
            withDestinationPath: cycleAlias.lastPathComponent
        )
        defer { try? FileManager.default.removeItem(at: root) }
        let expectedPath = try await GitWorktreeService.canonicalLocalPaths(
            [missingTarget.path],
            deadline: OperationDeadline(timeout: 10)
        )[0]

        let paths = try await GitWorktreeService.canonicalLocalPaths(
            [absoluteAlias.path, relativeAlias.path, cycleAlias.path],
            deadline: OperationDeadline(timeout: 10)
        )

        #expect(paths[0] == expectedPath)
        #expect(paths[1] == expectedPath)
        #expect(paths[2] == cycleAlias.path)
    }

    @Test("remote worktree paths expand home and resolve relative to the repository")
    func expandsRemoteWorktreePaths() {
        #expect(GitWorktreeService.expandedRemotePath(
            "~/app-feature",
            repoPath: "~/app",
            homePath: "/home/test"
        ) == "/home/test/app-feature")
        #expect(GitWorktreeService.expandedRemotePath(
            "../app-feature",
            repoPath: "/srv/repos/app",
            homePath: "/home/test"
        ) == "/srv/repos/app-feature")
        #expect(GitWorktreeService.expandedRemotePath(
            "/srv/repos/app-feature",
            repoPath: "/srv/repos/app",
            homePath: "/home/test"
        ) == "/srv/repos/app-feature")
    }

    @Test("remote path resolution physicalizes missing leaves through symlinked parents")
    func resolvesRemoteMissingLeavesThroughSymlinkedParents() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-remote-resolution-\(UUID().uuidString)", isDirectory: true)
        let physicalParent = root.appendingPathComponent("physical", isDirectory: true)
        let aliasParent = root.appendingPathComponent("alias")
        try FileManager.default.createDirectory(at: physicalParent, withIntermediateDirectories: true)
        try FileManager.default.createSymbolicLink(at: aliasParent, withDestinationURL: physicalParent)
        defer { try? FileManager.default.removeItem(at: root) }
        let requestedPath = aliasParent.appendingPathComponent("missing-worktree").path
        let expectedPath = try await GitWorktreeService.canonicalLocalPaths(
            [physicalParent.appendingPathComponent("missing-worktree").path],
            deadline: OperationDeadline(timeout: 10)
        )[0]

        let result = try await SubprocessRunner.run(SubprocessRequest(
            executablePath: "/bin/sh",
            arguments: ["-c", GitWorktreeService.remotePathResolutionCommand([requestedPath])],
            timeout: 10,
            outputByteLimit: 1024 * 1024
        ))
        let paths = try GitWorktreeService.decodeCanonicalPathOutput(
            result.stdoutData,
            expectedCount: 1
        )

        #expect(result.status == 0)
        #expect(!result.truncated)
        #expect(ProjectPickerPathService.standardizedRemotePath(paths[0]) == expectedPath)
    }

    @Test("remote path resolution anchors relative candidates to the shell working directory")
    func resolvesRemoteRelativeCandidates() async throws {
        let relativePath = "muxy-remote-relative-\(UUID().uuidString)/missing-worktree"
        let expectedPath = try await GitWorktreeService.canonicalLocalPaths(
            [relativePath],
            deadline: OperationDeadline(timeout: 10)
        )[0]

        let result = try await SubprocessRunner.run(SubprocessRequest(
            executablePath: "/bin/sh",
            arguments: ["-c", GitWorktreeService.remotePathResolutionCommand([relativePath])],
            timeout: 10,
            outputByteLimit: 1024 * 1024
        ))
        let paths = try GitWorktreeService.decodeCanonicalPathOutput(
            result.stdoutData,
            expectedCount: 1
        )

        #expect(result.status == 0)
        #expect(!result.truncated)
        #expect(ProjectPickerPathService.standardizedRemotePath(paths[0]) == expectedPath)
    }

    @Test("remote path resolution follows dangling symlink targets with bounded cycles")
    func resolvesRemoteDanglingSymlinkTargets() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-remote-dangling-\(UUID().uuidString)", isDirectory: true)
        let physicalParent = root.appendingPathComponent("physical", isDirectory: true)
        let missingTarget = physicalParent.appendingPathComponent("missing-worktree")
        let absoluteAlias = root.appendingPathComponent("absolute-alias")
        let relativeAlias = root.appendingPathComponent("relative-alias")
        let cycleAlias = root.appendingPathComponent("cycle-alias")
        try FileManager.default.createDirectory(at: physicalParent, withIntermediateDirectories: true)
        try FileManager.default.createSymbolicLink(at: absoluteAlias, withDestinationURL: missingTarget)
        try FileManager.default.createSymbolicLink(
            atPath: relativeAlias.path,
            withDestinationPath: "physical/missing-worktree"
        )
        try FileManager.default.createSymbolicLink(
            atPath: cycleAlias.path,
            withDestinationPath: cycleAlias.lastPathComponent
        )
        defer { try? FileManager.default.removeItem(at: root) }
        let expectedPath = try await GitWorktreeService.canonicalLocalPaths(
            [missingTarget.path],
            deadline: OperationDeadline(timeout: 10)
        )[0]

        let result = try await SubprocessRunner.run(SubprocessRequest(
            executablePath: "/bin/sh",
            arguments: [
                "-c",
                GitWorktreeService.remotePathResolutionCommand([
                    absoluteAlias.path,
                    relativeAlias.path,
                    cycleAlias.path,
                ]),
            ],
            timeout: 10,
            outputByteLimit: 1024 * 1024
        ))
        let paths = try GitWorktreeService.decodeCanonicalPathOutput(
            result.stdoutData,
            expectedCount: 3
        )

        #expect(result.status == 0)
        #expect(!result.truncated)
        #expect(ProjectPickerPathService.standardizedRemotePath(paths[0]) == expectedPath)
        #expect(ProjectPickerPathService.standardizedRemotePath(paths[1]) == expectedPath)
        #expect(paths[2] == cycleAlias.path)
    }

    @Test("heals an orphaned worktree referenced through a symlinked parent")
    func healsOrphanThroughSymlinkedParent() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1", message: "base")
        let worktreePath = repo.siblingPath("symlink-wt")
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repo.path,
            path: worktreePath,
            branch: "feature",
            createBranch: true,
            baseBranch: nil
        )
        try FileManager.default.removeItem(atPath: worktreePath)
        try repo.orphanWorktreeAdmin(named: "symlink-wt")

        let aliasPath = try repo.symlinkedSiblingPath(for: "symlink-wt")
        try await GitWorktreeService.shared.removeWorktree(repoPath: repo.path, path: aliasPath, force: true)

        let records = try await GitWorktreeService.shared.listWorktrees(repoPath: repo.path)
        #expect(!records.contains { GitWorktreeService.canonicalPath($0.path) == GitWorktreeService.canonicalPath(worktreePath) })
    }

    @Test("presence checks distinguish absence from inaccessible paths")
    func presenceChecksDistinguishAbsenceFromInaccessiblePaths() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-presence-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let link = root.appendingPathComponent("dangling")
        try FileManager.default.createSymbolicLink(
            at: link,
            withDestinationURL: root.appendingPathComponent("missing")
        )

        #expect(await LocalFileOps().exists(at: link.path))
        #expect(try await LocalFileOps().exists(at: link.path, timeout: 1))
        let absent = root.appendingPathComponent("missing/absent").path
        #expect(try await !LocalFileOps().exists(at: absent, timeout: 1))
        #expect(try await generatedRemotePresenceStatus(at: absent) == 1)

        let restricted = root.appendingPathComponent("restricted", isDirectory: true)
        let hidden = restricted.appendingPathComponent("deeper/target", isDirectory: true)
        let restrictedLink = root.appendingPathComponent("restricted-link")
        try FileManager.default.createDirectory(at: hidden, withIntermediateDirectories: true)
        try FileManager.default.createSymbolicLink(
            at: restrictedLink,
            withDestinationURL: restricted.appendingPathComponent("deeper", isDirectory: true)
        )
        try FileManager.default.setAttributes([.posixPermissions: 0], ofItemAtPath: restricted.path)
        defer { try? FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: restricted.path) }
        #expect(try await LocalFileOps().exists(at: restricted.appendingPathComponent("unknown").path, timeout: 1))
        #expect(try await LocalFileOps().exists(at: hidden.path, timeout: 1))
        #expect(try await generatedRemotePresenceStatus(at: hidden.path) == 2)
        let linkedHidden = restrictedLink.appendingPathComponent("target").path
        #expect(try await LocalFileOps().exists(at: linkedHidden, timeout: 1))
        #expect(try await generatedRemotePresenceStatus(at: linkedHidden) == 2)
    }
}

private func generatedRemotePresenceStatus(at path: String) async throws -> Int32 {
    try await SubprocessRunner.run(SubprocessRequest(
        executablePath: "/bin/sh",
        arguments: ["-c", FilePresenceCommand.remote(path: path)],
        timeout: 1
    )).status
}

private struct TempGitRepo {
    let path: String
    private let parent: String

    init() throws {
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-worktree-remove-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
        parent = base.path
        path = base.appendingPathComponent("repo", isDirectory: true).path
        try FileManager.default.createDirectory(atPath: path, withIntermediateDirectories: true)
        try run("init", "-q", "-b", "main")
        try run("config", "user.email", "test@example.com")
        try run("config", "user.name", "Test")
        try run("config", "commit.gpgsign", "false")
    }

    func cleanup() {
        try? FileManager.default.removeItem(atPath: parent)
    }

    func siblingPath(_ name: String) -> String {
        URL(fileURLWithPath: parent).appendingPathComponent(name).path
    }

    func commit(file: String, contents: String, message: String) throws {
        let fileURL = URL(fileURLWithPath: path).appendingPathComponent(file)
        try contents.write(to: fileURL, atomically: true, encoding: .utf8)
        try run("add", file)
        try run("commit", "-q", "-m", message)
    }

    func symlinkedSiblingPath(for name: String) throws -> String {
        let realParent = URL(fileURLWithPath: parent)
        let aliasParent = realParent.appendingPathComponent("alias-\(UUID().uuidString)")
        try FileManager.default.createSymbolicLink(at: aliasParent, withDestinationURL: realParent)
        return aliasParent.appendingPathComponent(name).path
    }

    func branchExists(_ branch: String) -> Bool {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["git", "-C", path, "branch", "--list", "--format=%(refname:short)"]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = Pipe()
        guard (try? process.run()) != nil else { return false }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        let output = String(data: data, encoding: .utf8) ?? ""
        return output.split(separator: "\n").map { $0.trimmingCharacters(in: .whitespaces) }.contains(branch)
    }

    func orphanWorktreeAdmin(named name: String) throws {
        let gitdir = URL(fileURLWithPath: path)
            .appendingPathComponent(".git/worktrees/\(name)/gitdir")
        try "/nonexistent/\(name)/.git\n".write(to: gitdir, atomically: true, encoding: .utf8)
    }

    func run(_ args: String...) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["git", "-C", path] + args
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = pipe
        try process.run()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            throw NSError(
                domain: "GitTestRepo",
                code: Int(process.terminationStatus),
                userInfo: [NSLocalizedDescriptionKey: String(data: data, encoding: .utf8) ?? ""]
            )
        }
    }
}

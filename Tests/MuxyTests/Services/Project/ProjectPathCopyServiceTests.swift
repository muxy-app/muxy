import Foundation
import Testing

@testable import Muxy

@Suite("ProjectPathResolver")
struct ProjectPathResolverTests {
    private enum TestError: Error {
        case unexpectedInvocation
    }

    @Test("local paths expand the home directory and standardize components")
    func localPathIsAbsolute() async throws {
        let resolver = ProjectPathResolver { _, _ in
            throw TestError.unexpectedInvocation
        }

        let path = try await resolver.absolutePath(for: "~/Projects/../Muxy", in: .local)

        let expected = URL(
            fileURLWithPath: ("~/Projects/../Muxy" as NSString).expandingTildeInPath
        ).standardizedFileURL.path
        #expect(path == expected)
        #expect(path.hasPrefix("/"))
    }

    @Test("absolute remote paths are standardized without an SSH command")
    func absoluteRemotePathDoesNotUseSSH() async throws {
        let resolver = ProjectPathResolver { _, _ in
            throw TestError.unexpectedInvocation
        }
        let destination = SSHDestination(host: "example.com")

        let path = try await resolver.absolutePath(for: "/srv/code/../api", in: .ssh(destination))

        #expect(path == "/srv/api")
    }

    @Test("home-relative remote paths resolve through the remote shell")
    func homeRelativeRemotePathUsesSSH() async throws {
        let destination = SSHDestination(host: "example.com", user: "alice")
        let resolver = ProjectPathResolver { receivedDestination, command in
            #expect(receivedDestination == destination)
            #expect(command == "cd ~/'code/api server' && pwd -P")
            return Self.result(stdout: "/home/alice/code/api server\n")
        }

        let path = try await resolver.absolutePath(for: "~/code/api server", in: .ssh(destination))

        #expect(path == "/home/alice/code/api server")
    }

    @Test("relative remote paths resolve through the remote shell")
    func relativeRemotePathUsesSSH() async throws {
        let destination = SSHDestination(host: "example.com")
        let resolver = ProjectPathResolver { _, command in
            #expect(command == "cd 'repo'\\''s' && pwd -P")
            return Self.result(stdout: "/home/user/repo's\n")
        }

        let path = try await resolver.absolutePath(for: "repo's", in: .ssh(destination))

        #expect(path == "/home/user/repo's")
    }

    @Test("remote command failures preserve the diagnostic")
    func remoteCommandFailureThrows() async {
        let destination = SSHDestination(host: "example.com")
        let resolver = ProjectPathResolver { _, _ in
            Self.result(status: 1, stderr: "permission denied\n")
        }

        do {
            _ = try await resolver.absolutePath(for: "~/private", in: .ssh(destination))
            Issue.record("Expected the remote command to fail")
        } catch let error as ProjectPathResolutionError {
            #expect(error.errorDescription == "permission denied")
        } catch {
            Issue.record("Unexpected error: \(error)")
        }
    }

    @Test("remote output must contain one absolute path")
    func invalidRemoteOutputThrows() async {
        let destination = SSHDestination(host: "example.com")
        let resolver = ProjectPathResolver { _, _ in
            Self.result(stdout: "startup output\n/home/user/repo\n")
        }

        await #expect(throws: ProjectPathResolutionError.self) {
            try await resolver.absolutePath(for: "~/repo", in: .ssh(destination))
        }
    }

    @Test("empty paths are rejected")
    func emptyPathThrows() async {
        let resolver = ProjectPathResolver { _, _ in
            throw TestError.unexpectedInvocation
        }

        await #expect(throws: ProjectPathResolutionError.self) {
            try await resolver.absolutePath(for: "", in: .local)
        }
    }

    private static func result(
        status: Int32 = 0,
        stdout: String = "",
        stderr: String = ""
    ) -> GitProcessResult {
        GitProcessResult(
            status: status,
            stdout: stdout,
            stdoutData: Data(stdout.utf8),
            stderr: stderr,
            truncated: false
        )
    }
}

import Foundation
import Testing

@testable import Muxy

@Suite("MuxyAPI.Tabs.resolveTabDirectory")
@MainActor
struct MuxyAPITabsResolveDirectoryTests {
    private var remoteContext: WorkspaceContext {
        .ssh(SSHDestination(host: "example.com", remoteRoot: "~/code"))
    }

    @Test("remote relative path resolves under the root")
    func remoteRelativePathResolves() {
        let resolved = MuxyAPI.Tabs.resolveTabDirectory(
            root: "~/code",
            relativePath: "api/src",
            context: remoteContext
        )

        #expect(resolved == "~/code/api/src")
    }

    @Test("remote empty relative path resolves to the root")
    func remoteEmptyRelativePathResolvesToRoot() {
        let resolved = MuxyAPI.Tabs.resolveTabDirectory(
            root: "~/code",
            relativePath: "",
            context: remoteContext
        )

        #expect(resolved == "~/code")
    }

    @Test("remote path escaping the root is rejected")
    func remotePathEscapingRootRejected() {
        let resolved = MuxyAPI.Tabs.resolveTabDirectory(
            root: "~/code",
            relativePath: "../secrets",
            context: remoteContext
        )

        #expect(resolved == nil)
    }

    @Test("remote path traversing back to the root is rejected")
    func remoteSiblingPrefixRejected() {
        let resolved = MuxyAPI.Tabs.resolveTabDirectory(
            root: "~/code",
            relativePath: "../code-private",
            context: remoteContext
        )

        #expect(resolved == nil)
    }
}

import Foundation
import Testing

@testable import Muxy

@Suite("GitRepositoryService.webURL(fromRemoteURL:)")
struct GitRemoteWebURLTests {
    @Test("https URL passes through and strips .git")
    func httpsURL() {
        let url = GitRepositoryService.webURL(fromRemoteURL: "https://github.com/foo/bar.git")
        #expect(url?.absoluteString == "https://github.com/foo/bar")
    }

    @Test("scp-style ssh URL converts to https")
    func scpStyle() {
        let url = GitRepositoryService.webURL(fromRemoteURL: "git@github.com:foo/bar.git")
        #expect(url?.absoluteString == "https://github.com/foo/bar")
    }

    @Test("ssh:// URL converts to https and drops user/port")
    func sshURL() {
        let url = GitRepositoryService.webURL(fromRemoteURL: "ssh://git@github.com:22/foo/bar.git")
        #expect(url?.absoluteString == "https://github.com/foo/bar")
    }

    @Test("empty string returns nil")
    func empty() {
        #expect(GitRepositoryService.webURL(fromRemoteURL: "") == nil)
    }

    @Test("gitlab https without .git is preserved")
    func gitlab() {
        let url = GitRepositoryService.webURL(fromRemoteURL: "https://gitlab.com/group/sub/proj")
        #expect(url?.absoluteString == "https://gitlab.com/group/sub/proj")
    }
}

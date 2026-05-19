import Foundation
import Testing

@testable import Muxy

@Suite("GitProcessRunner.gitHubCredentialHelperArgs")
struct GitProcessRunnerCredentialHelperTests {
    @Test("returns empty when gh is not on disk")
    func ghMissing() {
        let args = GitProcessRunner.gitHubCredentialHelperArgs { _ in nil }
        #expect(args.isEmpty)
    }

    @Test("resets inherited helper and scopes gh to github.com only")
    func ghPresent() {
        let args = GitProcessRunner.gitHubCredentialHelperArgs { _ in "/opt/homebrew/bin/gh" }
        #expect(args == [
            "-c", "credential.helper=",
            "-c", "credential.https://github.com.helper=!/opt/homebrew/bin/gh auth git-credential",
        ])
    }

    @Test("uses the absolute path returned by the resolver")
    func usesResolvedPath() {
        let args = GitProcessRunner.gitHubCredentialHelperArgs { _ in "/usr/local/bin/gh" }
        #expect(args.contains("credential.https://github.com.helper=!/usr/local/bin/gh auth git-credential"))
    }

    @Test("classifies git commands by timeout budget")
    func classifiesGitCommandTimeouts() {
        #expect(GitCommandClass.classifyGit(arguments: ["rev-parse", "--abbrev-ref", "HEAD"]) == .fastMetadata)
        #expect(GitCommandClass.classifyGit(arguments: ["-c", "core.quotepath=false", "status", "--porcelain=1"]) == .statusDiff)
        #expect(GitCommandClass.classifyGit(arguments: ["diff", "--cached"]) == .statusDiff)
        #expect(GitCommandClass.classifyGit(arguments: ["fetch", "origin"]) == .network)
        #expect(GitCommandClass.classifyGit(arguments: ["push"]) == .network)
        #expect(GitCommandClass.classifyGit(arguments: ["add", "--", "README.md"]) == .statusDiff)
    }
}

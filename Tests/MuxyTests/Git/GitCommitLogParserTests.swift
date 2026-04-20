import Foundation
import Testing

@testable import Muxy

@Suite("GitCommitLogParser")
struct GitCommitLogParserTests {
    private static let field = GitCommitLogParser.fieldSeparator
    private static let record = GitCommitLogParser.recordSeparator

    private func makeRecord(
        hash: String = "abcdef1234567890abcdef1234567890abcdef12",
        shortHash: String = "abcdef1",
        subject: String = "Initial commit",
        author: String = "Alice",
        date: String = "2025-01-15T10:30:00Z",
        refs: String = "",
        parents: String = ""
    ) -> String {
        [hash, shortHash, subject, author, date, refs, parents]
            .joined(separator: Self.field) + Self.record
    }

    @Test("empty input returns empty array")
    func empty() {
        #expect(GitCommitLogParser.parseCommitLog("").isEmpty)
    }

    @Test("single record parses all fields")
    func singleRecord() {
        let raw = makeRecord(
            hash: "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0",
            shortHash: "a1b2c3d",
            subject: "Fix bug",
            author: "Bob Builder",
            date: "2025-03-10T08:00:00Z",
            refs: "",
            parents: "parent1hash parent2hash"
        )
        let commits = GitCommitLogParser.parseCommitLog(raw)
        #expect(commits.count == 1)
        let c = commits[0]
        #expect(c.hash == "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0")
        #expect(c.shortHash == "a1b2c3d")
        #expect(c.subject == "Fix bug")
        #expect(c.authorName == "Bob Builder")
        #expect(c.parentHashes == ["parent1hash", "parent2hash"])
        #expect(c.isMerge == true)
    }

    @Test("multiple records parse in order")
    func multipleRecords() {
        let raw = makeRecord(hash: "aaa", shortHash: "aaa", subject: "First")
            + makeRecord(hash: "bbb", shortHash: "bbb", subject: "Second")
            + makeRecord(hash: "ccc", shortHash: "ccc", subject: "Third")
        let commits = GitCommitLogParser.parseCommitLog(raw)
        #expect(commits.count == 3)
        #expect(commits.map(\.subject) == ["First", "Second", "Third"])
    }

    @Test("malformed record with too few fields is dropped")
    func malformedDropped() {
        let raw = "only\(Self.field)two\(Self.record)" + makeRecord(subject: "Good one")
        let commits = GitCommitLogParser.parseCommitLog(raw)
        #expect(commits.count == 1)
        #expect(commits[0].subject == "Good one")
    }

    @Test("single parent is non-merge")
    func singleParentNotMerge() {
        let raw = makeRecord(parents: "onlyone")
        let commits = GitCommitLogParser.parseCommitLog(raw)
        #expect(commits[0].isMerge == false)
        #expect(commits[0].parentHashes == ["onlyone"])
    }

    @Test("parseRefs handles empty string")
    func parseRefsEmpty() {
        #expect(GitCommitLogParser.parseRefs("").isEmpty)
    }

    @Test("parseRefs handles HEAD")
    func parseRefsHead() {
        let refs = GitCommitLogParser.parseRefs("HEAD")
        #expect(refs.count == 1)
        #expect(refs[0].name == "HEAD")
        #expect(refs[0].kind == .head)
    }

    @Test("parseRefs handles HEAD -> branch")
    func parseRefsHeadArrow() {
        let refs = GitCommitLogParser.parseRefs("HEAD -> refs/heads/main")
        #expect(refs.count == 1)
        #expect(refs[0].name == "main")
        #expect(refs[0].kind == .localBranch)
    }

    @Test("parseRefs handles tag prefix")
    func parseRefsTagPrefix() {
        let refs = GitCommitLogParser.parseRefs("tag: refs/tags/v1.0")
        #expect(refs.count == 1)
        #expect(refs[0].name == "v1.0")
        #expect(refs[0].kind == .tag)
    }

    @Test("parseRefs handles refs/heads prefix")
    func parseRefsLocalBranch() {
        let refs = GitCommitLogParser.parseRefs("refs/heads/feature")
        #expect(refs.count == 1)
        #expect(refs[0].name == "feature")
        #expect(refs[0].kind == .localBranch)
    }

    @Test("parseRefs handles refs/remotes prefix")
    func parseRefsRemoteBranch() {
        let refs = GitCommitLogParser.parseRefs("refs/remotes/origin/main")
        #expect(refs.count == 1)
        #expect(refs[0].name == "origin/main")
        #expect(refs[0].kind == .remoteBranch)
    }

    @Test("parseRefs handles multiple comma-separated refs")
    func parseRefsMultiple() {
        let refs = GitCommitLogParser.parseRefs("HEAD -> refs/heads/main, refs/remotes/origin/main, tag: refs/tags/v2.0")
        #expect(refs.count == 3)
        #expect(refs[0].kind == .localBranch)
        #expect(refs[1].kind == .remoteBranch)
        #expect(refs[2].kind == .tag)
        #expect(refs[0].name == "main")
        #expect(refs[1].name == "origin/main")
        #expect(refs[2].name == "v2.0")
    }

    @Test("parseRefs treats bare name as local branch")
    func parseRefsBareName() {
        let refs = GitCommitLogParser.parseRefs("develop")
        #expect(refs.count == 1)
        #expect(refs[0].name == "develop")
        #expect(refs[0].kind == .localBranch)
    }

    @Test("logFormat matches expected git format string")
    func logFormatShape() {
        #expect(GitCommitLogParser.logFormat.contains("%H"))
        #expect(GitCommitLogParser.logFormat.contains("%h"))
        #expect(GitCommitLogParser.logFormat.contains("%s"))
        #expect(GitCommitLogParser.logFormat.contains("%an"))
        #expect(GitCommitLogParser.logFormat.contains("%aI"))
        #expect(GitCommitLogParser.logFormat.contains("%D"))
        #expect(GitCommitLogParser.logFormat.contains("%P"))
        #expect(GitCommitLogParser.logFormat.hasSuffix(GitCommitLogParser.recordSeparator))
    }
}

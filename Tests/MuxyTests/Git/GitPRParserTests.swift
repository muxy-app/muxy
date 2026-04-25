import Foundation
import Testing

@testable import Muxy

@Suite("GitPRParser")
struct GitPRParserTests {
    @Suite("parseStatusChecks")
    struct StatusChecks {
        @Test("empty rollup returns none status")
        func emptyRollup() {
            let result = GitPRParser.parseStatusChecks([])
            #expect(result.status == .none)
            #expect(result.total == 0)
        }

        @Test("all successful check runs report success")
        func allSuccess() {
            let rollup: [[String: Any]] = [
                ["__typename": "CheckRun", "status": "COMPLETED", "conclusion": "SUCCESS"],
                ["__typename": "CheckRun", "status": "COMPLETED", "conclusion": "NEUTRAL"],
                ["__typename": "CheckRun", "status": "COMPLETED", "conclusion": "SKIPPED"],
            ]
            let result = GitPRParser.parseStatusChecks(rollup)
            #expect(result.status == .success)
            #expect(result.passing == 3)
            #expect(result.failing == 0)
            #expect(result.pending == 0)
        }

        @Test("any failure dominates status")
        func anyFailure() {
            let rollup: [[String: Any]] = [
                ["__typename": "CheckRun", "status": "COMPLETED", "conclusion": "SUCCESS"],
                ["__typename": "CheckRun", "status": "COMPLETED", "conclusion": "FAILURE"],
                ["__typename": "CheckRun", "status": "COMPLETED", "conclusion": "SUCCESS"],
            ]
            let result = GitPRParser.parseStatusChecks(rollup)
            #expect(result.status == .failure)
            #expect(result.passing == 2)
            #expect(result.failing == 1)
        }

        @Test("pending only reports pending status")
        func pendingOnly() {
            let rollup: [[String: Any]] = [
                ["__typename": "CheckRun", "status": "IN_PROGRESS"],
                ["__typename": "CheckRun", "status": "QUEUED"],
            ]
            let result = GitPRParser.parseStatusChecks(rollup)
            #expect(result.status == .pending)
            #expect(result.pending == 2)
        }

        @Test("StatusContext entries use state field")
        func statusContextUsesStateField() {
            let rollup: [[String: Any]] = [
                ["__typename": "StatusContext", "state": "SUCCESS"],
                ["__typename": "StatusContext", "state": "FAILURE"],
                ["__typename": "StatusContext", "state": "PENDING"],
            ]
            let result = GitPRParser.parseStatusChecks(rollup)
            #expect(result.passing == 1)
            #expect(result.failing == 1)
            #expect(result.pending == 1)
            #expect(result.status == .failure)
        }

        @Test("all failure-class conclusions classify as failing")
        func allFailureConclusions() {
            let conclusions = ["FAILURE", "ERROR", "CANCELLED", "TIMED_OUT", "ACTION_REQUIRED", "STARTUP_FAILURE"]
            let rollup = conclusions.map { c -> [String: Any] in
                ["__typename": "CheckRun", "status": "COMPLETED", "conclusion": c]
            }
            let result = GitPRParser.parseStatusChecks(rollup)
            #expect(result.failing == conclusions.count)
            #expect(result.passing == 0)
        }

        @Test("unknown conclusion falls into pending bucket")
        func unknownConclusion() {
            let rollup: [[String: Any]] = [
                ["__typename": "CheckRun", "status": "COMPLETED", "conclusion": "MYSTERY"],
            ]
            let result = GitPRParser.parseStatusChecks(rollup)
            #expect(result.pending == 1)
        }
    }

    @Suite("parsePRInfo")
    struct PRInfoParsing {
        @Test("full JSON parses all fields")
        func fullJSON() {
            let json = """
            {
              "url": "https://github.com/a/b/pull/42",
              "number": 42,
              "state": "OPEN",
              "isDraft": true,
              "baseRefName": "main",
              "mergeable": "MERGEABLE",
              "mergeStateStatus": "CLEAN",
              "statusCheckRollup": []
            }
            """
            let info = GitPRParser.parsePRInfo(json)
            #expect(info?.url == "https://github.com/a/b/pull/42")
            #expect(info?.number == 42)
            #expect(info?.state == .open)
            #expect(info?.isDraft == true)
            #expect(info?.baseBranch == "main")
            #expect(info?.mergeable == true)
            #expect(info?.mergeStateStatus == .clean)
            #expect(info?.checks.status == GitRepositoryService.PRChecksStatus.none)
        }

        @Test("BEHIND mergeStateStatus parses even when mergeable is MERGEABLE")
        func behindMergeState() {
            let json = """
            {"url":"u","number":1,"state":"OPEN","mergeable":"MERGEABLE","mergeStateStatus":"BEHIND"}
            """
            let info = GitPRParser.parsePRInfo(json)
            #expect(info?.mergeable == true)
            #expect(info?.mergeStateStatus == .behind)
        }

        @Test("missing mergeStateStatus defaults to unknown")
        func missingMergeState() {
            let json = #"{"url":"u","number":1,"state":"OPEN","mergeable":"MERGEABLE"}"#
            let info = GitPRParser.parsePRInfo(json)
            #expect(info?.mergeStateStatus == .unknown)
        }

        @Test("CONFLICTING mergeable maps to false")
        func conflictingMergeable() {
            let json = """
            {"url":"u","number":1,"state":"OPEN","mergeable":"CONFLICTING"}
            """
            let info = GitPRParser.parsePRInfo(json)
            #expect(info?.mergeable == false)
        }

        @Test("unknown mergeable maps to nil")
        func unknownMergeable() {
            let json = """
            {"url":"u","number":1,"state":"OPEN","mergeable":"UNKNOWN"}
            """
            let info = GitPRParser.parsePRInfo(json)
            #expect(info?.mergeable == nil)
        }

        @Test("missing required fields returns nil")
        func missingRequired() {
            #expect(GitPRParser.parsePRInfo("{}") == nil)
            #expect(GitPRParser.parsePRInfo(#"{"url":"u"}"#) == nil)
            #expect(GitPRParser.parsePRInfo(#"{"url":"u","number":1}"#) == nil)
        }

        @Test("invalid JSON returns nil")
        func invalidJSON() {
            #expect(GitPRParser.parsePRInfo("not json") == nil)
        }

        @Test("unknown state defaults to open")
        func unknownStateDefaults() {
            let json = #"{"url":"u","number":1,"state":"WAT"}"#
            let info = GitPRParser.parsePRInfo(json)
            #expect(info?.state == .open)
        }

        @Test("isCrossRepository true parses")
        func isCrossRepositoryTrue() {
            let json = #"{"url":"u","number":1,"state":"OPEN","isCrossRepository":true}"#
            let info = GitPRParser.parsePRInfo(json)
            #expect(info?.isCrossRepository == true)
        }

        @Test("isCrossRepository false parses")
        func isCrossRepositoryFalse() {
            let json = #"{"url":"u","number":1,"state":"OPEN","isCrossRepository":false}"#
            let info = GitPRParser.parsePRInfo(json)
            #expect(info?.isCrossRepository == false)
        }

        @Test("missing isCrossRepository defaults to false")
        func isCrossRepositoryMissing() {
            let json = #"{"url":"u","number":1,"state":"OPEN"}"#
            let info = GitPRParser.parsePRInfo(json)
            #expect(info?.isCrossRepository == false)
        }
    }

    @Suite("parsePRInfoMatchingHeadSha")
    struct PRInfoMatchingHeadSha {
        @Test("matches PR by head SHA case-insensitively")
        func matches() {
            let json = """
            [
              {
                "url": "https://github.com/o/r/pull/1",
                "number": 1,
                "state": "OPEN",
                "baseRefName": "main",
                "statusCheckRollup": [],
                "headRefOid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
              },
              {
                "url": "https://github.com/o/r/pull/2",
                "number": 2,
                "state": "OPEN",
                "baseRefName": "main",
                "statusCheckRollup": [],
                "headRefOid": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
              }
            ]
            """
            let info = GitPRParser.parsePRInfoMatchingHeadSha(
                json,
                headSha: "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
            )
            #expect(info?.number == 2)
        }

        @Test("returns nil when no PR matches")
        func noMatch() {
            let json = """
            [{"url":"u","number":1,"state":"OPEN","statusCheckRollup":[],"headRefOid":"aa"}]
            """
            #expect(GitPRParser.parsePRInfoMatchingHeadSha(json, headSha: "deadbeef") == nil)
        }

        @Test("returns nil for invalid JSON")
        func invalid() {
            #expect(GitPRParser.parsePRInfoMatchingHeadSha("not-json", headSha: "x") == nil)
        }
    }

    @Suite("parseAheadBehind")
    struct AheadBehindParsing {
        @Test("no upstream returns zeros")
        func noUpstream() {
            let result = GitPRParser.parseAheadBehind(counts: "", hasUpstream: false)
            #expect(result.hasUpstream == false)
            #expect(result.ahead == 0)
            #expect(result.behind == 0)
        }

        @Test("tab-separated counts parse")
        func tabSeparated() {
            let result = GitPRParser.parseAheadBehind(counts: "3\t5\n", hasUpstream: true)
            #expect(result.hasUpstream == true)
            #expect(result.ahead == 3)
            #expect(result.behind == 5)
        }

        @Test("space-separated counts parse")
        func spaceSeparated() {
            let result = GitPRParser.parseAheadBehind(counts: "7 2", hasUpstream: true)
            #expect(result.ahead == 7)
            #expect(result.behind == 2)
        }

        @Test("malformed counts fall back to zeros with upstream")
        func malformed() {
            let result = GitPRParser.parseAheadBehind(counts: "abc", hasUpstream: true)
            #expect(result.hasUpstream == true)
            #expect(result.ahead == 0)
            #expect(result.behind == 0)
        }
    }

    @Suite("parsePRList")
    struct PRList {
        @Test("well-formed list parses all fields")
        func wellFormed() {
            let json = """
            [
              {
                "number": 42,
                "title": "Add feature",
                "author": {"login": "alice"},
                "headRefName": "feature/x",
                "baseRefName": "main",
                "state": "OPEN",
                "isDraft": false,
                "url": "https://github.com/o/r/pull/42",
                "updatedAt": "2026-04-01T12:34:56Z",
                "statusCheckRollup": [
                  {"__typename": "CheckRun", "status": "COMPLETED", "conclusion": "SUCCESS"}
                ]
              }
            ]
            """
            let items = GitPRParser.parsePRList(json)
            #expect(items.count == 1)
            let item = items[0]
            #expect(item.number == 42)
            #expect(item.title == "Add feature")
            #expect(item.author == "alice")
            #expect(item.headBranch == "feature/x")
            #expect(item.baseBranch == "main")
            #expect(item.state == .open)
            #expect(item.isDraft == false)
            #expect(item.url == "https://github.com/o/r/pull/42")
            #expect(item.updatedAt != nil)
            #expect(item.checks.status == .success)
            #expect(item.checks.passing == 1)
        }

        @Test("empty array returns empty list")
        func emptyArray() {
            #expect(GitPRParser.parsePRList("[]").isEmpty)
        }

        @Test("malformed JSON returns empty list")
        func malformed() {
            #expect(GitPRParser.parsePRList("not-json").isEmpty)
            #expect(GitPRParser.parsePRList("").isEmpty)
            #expect(GitPRParser.parsePRList("{\"not\": \"array\"}").isEmpty)
        }

        @Test("missing optional fields fall back to defaults")
        func missingOptionals() {
            let json = """
            [
              {"number": 7, "title": "Minimal"}
            ]
            """
            let items = GitPRParser.parsePRList(json)
            #expect(items.count == 1)
            let item = items[0]
            #expect(item.number == 7)
            #expect(item.title == "Minimal")
            #expect(item.author == "")
            #expect(item.headBranch == "")
            #expect(item.baseBranch == "")
            #expect(item.state == .open)
            #expect(item.isDraft == false)
            #expect(item.url == "")
            #expect(item.updatedAt == nil)
            #expect(item.checks.status == .none)
        }

        @Test("entries missing required fields are skipped")
        func skipsInvalidEntries() {
            let json = """
            [
              {"title": "no number"},
              {"number": 9},
              {"number": 10, "title": "kept"}
            ]
            """
            let items = GitPRParser.parsePRList(json)
            #expect(items.count == 1)
            #expect(items[0].number == 10)
        }

        @Test("unknown state falls back to open")
        func unknownStateFallsBack() {
            let json = """
            [{"number": 1, "title": "t", "state": "WEIRD"}]
            """
            let items = GitPRParser.parsePRList(json)
            #expect(items[0].state == .open)
        }

        @Test("merged and closed states parse")
        func mergedAndClosed() {
            let json = """
            [
              {"number": 1, "title": "m", "state": "MERGED"},
              {"number": 2, "title": "c", "state": "CLOSED"}
            ]
            """
            let items = GitPRParser.parsePRList(json)
            #expect(items[0].state == .merged)
            #expect(items[1].state == .closed)
        }

        @Test("headRefOid and merge fields parse")
        func headRefOidAndMergeFields() {
            let json = """
            [
              {
                "number": 7,
                "title": "Hello",
                "author": {"login": "alice"},
                "headRefName": "feature",
                "headRefOid": "cccccccccccccccccccccccccccccccccccccccc",
                "baseRefName": "main",
                "state": "OPEN",
                "mergeable": "CONFLICTING",
                "mergeStateStatus": "DIRTY"
              }
            ]
            """
            let items = GitPRParser.parsePRList(json)
            #expect(items.count == 1)
            #expect(items[0].headRefOid == "cccccccccccccccccccccccccccccccccccccccc")
            #expect(items[0].mergeable == false)
            #expect(items[0].mergeStateStatus == .dirty)
        }

        @Test("updatedAt parses with and without fractional seconds")
        func updatedAtFormats() {
            let json = """
            [
              {"number": 1, "title": "a", "updatedAt": "2026-04-01T12:00:00Z"},
              {"number": 2, "title": "b", "updatedAt": "2026-04-01T12:00:00.123Z"}
            ]
            """
            let items = GitPRParser.parsePRList(json)
            #expect(items[0].updatedAt != nil)
            #expect(items[1].updatedAt != nil)
        }
    }
}

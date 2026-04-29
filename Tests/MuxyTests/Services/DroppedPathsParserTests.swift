import Foundation
import Testing

@testable import Muxy

@Suite("DroppedPathsParser")
struct DroppedPathsParserTests {
    @Test("file URLs are returned as filesystem paths")
    func fileURLs() {
        let urls = [URL(fileURLWithPath: "/tmp/a.txt"), URL(fileURLWithPath: "/tmp/b.txt")]
        #expect(DroppedPathsParser.parse(fileURLs: urls, plainString: nil) == ["/tmp/a.txt", "/tmp/b.txt"])
    }

    @Test("non-file URLs are filtered out")
    func nonFileURLs() {
        let urls = [URL(string: "https://example.com")!, URL(fileURLWithPath: "/tmp/a.txt")]
        #expect(DroppedPathsParser.parse(fileURLs: urls, plainString: nil) == ["/tmp/a.txt"])
    }

    @Test("empty inputs return empty")
    func empty() {
        #expect(DroppedPathsParser.parse(fileURLs: [], plainString: nil).isEmpty)
        #expect(DroppedPathsParser.parse(fileURLs: [], plainString: "").isEmpty)
    }

    @Test("file:// strings are decoded to paths")
    func fileURLString() {
        let result = DroppedPathsParser.parse(
            fileURLs: [],
            plainString: "file:///tmp/a.txt",
            fileExists: { _ in false }
        )
        #expect(result == ["/tmp/a.txt"])
    }

    @Test("absolute paths that exist on disk are accepted")
    func absolutePathExists() {
        let result = DroppedPathsParser.parse(
            fileURLs: [],
            plainString: "/tmp/a.txt",
            fileExists: { $0 == "/tmp/a.txt" }
        )
        #expect(result == ["/tmp/a.txt"])
    }

    @Test("absolute paths that do not exist are rejected as a whole drop")
    func absolutePathMissing() {
        let result = DroppedPathsParser.parse(
            fileURLs: [],
            plainString: "/tmp/missing.txt",
            fileExists: { _ in false }
        )
        #expect(result.isEmpty)
    }

    @Test("mixed valid and invalid lines reject the entire drop")
    func mixedRejected() {
        let result = DroppedPathsParser.parse(
            fileURLs: [],
            plainString: "/tmp/a.txt\nrandom log line\n/tmp/b.txt",
            fileExists: { _ in true }
        )
        #expect(result.isEmpty)
    }

    @Test("multiple valid lines all accepted")
    func multipleValid() {
        let result = DroppedPathsParser.parse(
            fileURLs: [],
            plainString: "/tmp/a.txt\nfile:///tmp/b.txt",
            fileExists: { _ in true }
        )
        #expect(result == ["/tmp/a.txt", "/tmp/b.txt"])
    }

    @Test("file URLs take precedence over plain string")
    func urlsPrecedence() {
        let result = DroppedPathsParser.parse(
            fileURLs: [URL(fileURLWithPath: "/tmp/a.txt")],
            plainString: "/tmp/b.txt",
            fileExists: { _ in true }
        )
        #expect(result == ["/tmp/a.txt"])
    }

    @Test("non-path text is rejected")
    func nonPathText() {
        let result = DroppedPathsParser.parse(
            fileURLs: [],
            plainString: "hello world",
            fileExists: { _ in true }
        )
        #expect(result.isEmpty)
    }

    @Test("whitespace around lines is trimmed")
    func trimmed() {
        let result = DroppedPathsParser.parse(
            fileURLs: [],
            plainString: "   /tmp/a.txt  \n  /tmp/b.txt  ",
            fileExists: { _ in true }
        )
        #expect(result == ["/tmp/a.txt", "/tmp/b.txt"])
    }
}

@Suite("VCSFileTree")
struct VCSFileTreeTests {
    @Test("folders are collapsed by default")
    func foldersCollapsedByDefault() {
        let files = [
            makeFile("App/Models/User.swift"),
            makeFile("App/Views/UserView.swift"),
            makeFile("README.md"),
        ]

        let rows = VCSFileTree.rows(files: files, expandedFolders: [])

        #expect(rows.count == 2)
        #expect(rows[0] == .folder(.init(path: "App", name: "App", depth: 0, fileCount: 2)))
        #expect(rows[1] == .file(makeFile("README.md"), depth: 0))
    }

    @Test("expanding a folder reveals nested content")
    func expandingFolderShowsChildren() {
        let files = [
            makeFile("Sources/Core/A.swift"),
            makeFile("Sources/UI/B.swift"),
        ]

        let rows = VCSFileTree.rows(files: files, expandedFolders: ["Sources", "Sources/Core"])

        #expect(rows.contains(.folder(.init(path: "Sources", name: "Sources", depth: 0, fileCount: 2))))
        #expect(rows.contains(.folder(.init(path: "Sources/Core", name: "Core", depth: 1, fileCount: 1))))
        #expect(rows.contains(.file(makeFile("Sources/Core/A.swift"), depth: 2)))
        #expect(rows.contains(.folder(.init(path: "Sources/UI", name: "UI", depth: 1, fileCount: 1))))
    }

    @Test("single-child folder chains are compacted into one row")
    func compactFolders() {
        let files = [makeFile("Muxy/Views/VCS/VCSTabView.swift")]

        let rows = VCSFileTree.rows(files: files, expandedFolders: [])

        #expect(rows.count == 1)
        #expect(rows[0] == .folder(.init(path: "Muxy/Views/VCS", name: "Muxy/Views/VCS", depth: 0, fileCount: 1)))
    }

    @Test("compact folder expands to show files at correct depth")
    func compactFolderExpanded() {
        let file = makeFile("Muxy/Views/VCS/VCSTabView.swift")
        let rows = VCSFileTree.rows(files: [file], expandedFolders: ["Muxy/Views/VCS"])

        #expect(rows.count == 2)
        #expect(rows[0] == .folder(.init(path: "Muxy/Views/VCS", name: "Muxy/Views/VCS", depth: 0, fileCount: 1)))
        #expect(rows[1] == .file(file, depth: 1))
    }

    @Test("compaction stops at branching point")
    func compactionStopsAtBranch() {
        let files = [
            makeFile("Muxy/Views/VCS/VCSTabView.swift"),
            makeFile("Muxy/Views/Editor/EditorView.swift"),
        ]

        let rows = VCSFileTree.rows(files: files, expandedFolders: [])

        #expect(rows.count == 1)
        #expect(rows[0] == .folder(.init(path: "Muxy/Views", name: "Muxy/Views", depth: 0, fileCount: 2)))
    }

    @Test("folders and files are sorted case-insensitively")
    func sortedRows() {
        let files = [
            makeFile("zeta/file.swift"),
            makeFile("Alpha/file.swift"),
            makeFile("beta.swift"),
        ]

        let rows = VCSFileTree.rows(files: files, expandedFolders: [])

        #expect(rows[0] == .folder(.init(path: "Alpha", name: "Alpha", depth: 0, fileCount: 1)))
        #expect(rows[1] == .folder(.init(path: "zeta", name: "zeta", depth: 0, fileCount: 1)))
        #expect(rows[2] == .file(makeFile("beta.swift"), depth: 0))
    }

    private func makeFile(_ path: String) -> GitStatusFile {
        GitStatusFile(
            path: path,
            oldPath: nil,
            xStatus: "M",
            yStatus: " ",
            additions: nil,
            deletions: nil,
            isBinary: false
        )
    }
}



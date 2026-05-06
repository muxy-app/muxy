import Foundation
import Testing

@testable import Muxy

@Suite("EditorTabPathMigrator")
@MainActor
struct EditorTabPathMigratorTests {
    private func makeTempProject() -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func makeEditorTab(projectPath: String, filePath: String) throws -> TerminalTab {
        try "".write(toFile: filePath, atomically: true, encoding: .utf8)
        let state = EditorTabState(projectPath: projectPath, filePath: filePath)
        return TerminalTab(editorState: state)
    }

    private func makeWorkspaceRoots(tabs: [TerminalTab], projectPath: String) -> [WorktreeKey: SplitNode] {
        let area = TabArea(projectPath: projectPath, existingTab: tabs[0])
        for tab in tabs.dropFirst() {
            area.tabs.append(tab)
        }
        let key = WorktreeKey(projectID: UUID(), worktreeID: UUID())
        return [key: .tabArea(area)]
    }

    @Test("renames an exact-match editor tab")
    func renamesExactMatch() throws {
        let project = makeTempProject()
        defer { try? FileManager.default.removeItem(at: project) }

        let oldPath = project.appendingPathComponent("old.txt").path
        let tab = try makeEditorTab(projectPath: project.path, filePath: oldPath)
        let roots = makeWorkspaceRoots(tabs: [tab], projectPath: project.path)

        let newPath = project.appendingPathComponent("new.txt").path
        EditorTabPathMigrator.applyFileMove(from: oldPath, to: newPath, in: roots)

        #expect(tab.content.editorState?.filePath == newPath)
    }

    @Test("rewrites tabs whose paths are inside a renamed directory")
    func rewritesNestedPaths() throws {
        let project = makeTempProject()
        defer { try? FileManager.default.removeItem(at: project) }

        let oldDir = project.appendingPathComponent("src")
        try FileManager.default.createDirectory(at: oldDir, withIntermediateDirectories: true)
        let nested = oldDir.appendingPathComponent("a.txt").path
        let tab = try makeEditorTab(projectPath: project.path, filePath: nested)
        let roots = makeWorkspaceRoots(tabs: [tab], projectPath: project.path)

        let newDir = project.appendingPathComponent("lib").path
        EditorTabPathMigrator.applyFileMove(from: oldDir.path, to: newDir, in: roots)

        #expect(tab.content.editorState?.filePath == newDir + "/a.txt")
    }

    @Test("ignores tabs whose paths only share a prefix without a path boundary")
    func ignoresPrefixWithoutBoundary() throws {
        let project = makeTempProject()
        defer { try? FileManager.default.removeItem(at: project) }

        let foo = project.appendingPathComponent("foo.txt").path
        let foobar = project.appendingPathComponent("foobar.txt").path
        let fooTab = try makeEditorTab(projectPath: project.path, filePath: foo)
        let foobarTab = try makeEditorTab(projectPath: project.path, filePath: foobar)
        let roots = makeWorkspaceRoots(tabs: [fooTab, foobarTab], projectPath: project.path)

        let movedFoo = project.appendingPathComponent("renamed.txt").path
        EditorTabPathMigrator.applyFileMove(from: foo, to: movedFoo, in: roots)

        #expect(fooTab.content.editorState?.filePath == movedFoo)
        #expect(foobarTab.content.editorState?.filePath == foobar)
    }

    @Test("no-op when oldPath equals newPath")
    func noOpForIdenticalPaths() throws {
        let project = makeTempProject()
        defer { try? FileManager.default.removeItem(at: project) }

        let path = project.appendingPathComponent("x.txt").path
        let tab = try makeEditorTab(projectPath: project.path, filePath: path)
        let roots = makeWorkspaceRoots(tabs: [tab], projectPath: project.path)

        EditorTabPathMigrator.applyFileMove(from: path, to: path, in: roots)

        #expect(tab.content.editorState?.filePath == path)
    }
}

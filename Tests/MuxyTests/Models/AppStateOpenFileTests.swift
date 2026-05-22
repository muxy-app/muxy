import Foundation
import Testing

@testable import Muxy

@Suite("AppState.openFile")
@MainActor
struct AppStateOpenFileTests {
    @Test("openFile opens svg files in the editor html preview")
    func openFileOpensSVGFilesInEditorHTMLPreview() throws {
        let tempDirectory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
        let fileURL = tempDirectory.appendingPathComponent("icon.svg")
        try "<svg></svg>".write(to: fileURL, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let harness = makeHarness(projectPath: tempDirectory.path)

        harness.appState.openFile(fileURL.path, projectID: harness.projectID)

        let tab = harness.area.activeTab
        let editorState = tab?.content.editorState
        #expect(tab?.kind == .editor)
        #expect(tab?.content.imageViewerState == nil)
        #expect(editorState?.isSVGFile == true)
        #expect(editorState?.usesHTMLPreview == true)
        #expect(editorState?.htmlViewMode == .preview)
    }

    private func makeHarness(projectPath: String) -> Harness {
        let projectID = UUID()
        let worktreeID = UUID()
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: projectPath)
        let appState = AppState(
            selectionStore: OpenFileSelectionStoreStub(),
            terminalViews: OpenFileTerminalViewRemovingStub(),
            workspacePersistence: OpenFileWorkspacePersistenceStub()
        )
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return Harness(appState: appState, projectID: projectID, area: area)
    }

    private struct Harness {
        let appState: AppState
        let projectID: UUID
        let area: TabArea
    }
}

private final class OpenFileWorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class OpenFileSelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class OpenFileTerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}

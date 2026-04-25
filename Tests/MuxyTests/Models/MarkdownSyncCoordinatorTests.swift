import CoreGraphics
import Foundation
import Testing

@testable import Muxy

@Suite("MarkdownSyncCoordinator")
struct MarkdownSyncCoordinatorTests {
    @Test("editor scroll yields preview scroll target")
    @MainActor
    func editorToPreview() {
        let map = makeMap()
        let coordinator = MarkdownSyncCoordinator(now: { 0 })
        let output = coordinator.editorDidScroll(scrollY: 0, map: map)

        #expect(output.requestPreviewScrollTop != nil)
        #expect(output.requestEditorScrollY == nil)
    }

    @Test("preview scroll yields editor scroll target")
    @MainActor
    func previewToEditor() {
        let map = makeMap()
        let coordinator = MarkdownSyncCoordinator(now: { 0 })
        let output = coordinator.previewDidScroll(scrollTop: 0, map: map)

        #expect(output.requestPreviewScrollTop == nil)
        #expect(output.requestEditorScrollY != nil)
    }

    @Test("suppresses preview echo immediately after editor-driven request")
    @MainActor
    func suppressPreviewEcho() {
        var time: TimeInterval = 0
        let coordinator = MarkdownSyncCoordinator(now: { time })
        let map = makeMap()

        let firstOutput = coordinator.editorDidScroll(scrollY: 100, map: map)
        let echoTarget = firstOutput.requestPreviewScrollTop ?? 0

        time = 0.05
        let echo = coordinator.previewDidScroll(scrollTop: echoTarget, map: map)
        #expect(echo.isEmpty)
    }

    @Test("accepts later preview update outside suppression window")
    @MainActor
    func acceptsAfterWindow() {
        var time: TimeInterval = 0
        let coordinator = MarkdownSyncCoordinator(now: { time })
        let map = makeMap()

        _ = coordinator.editorDidScroll(scrollY: 100, map: map)

        time = 0.5
        let later = coordinator.previewDidScroll(scrollTop: 250, map: map)
        #expect(later.requestEditorScrollY != nil)
    }

    @Test("reissues preview target after relayout when editor is driver")
    @MainActor
    func relayoutReissue() {
        let coordinator = MarkdownSyncCoordinator(now: { 0 })
        let map = makeMap()

        _ = coordinator.editorDidScroll(scrollY: 50, map: map)
        let output = coordinator.reissueAfterRelayout(map: map)
        #expect(output.requestPreviewScrollTop != nil)
    }

    private func makeMap() -> MarkdownSyncMap {
        let anchors = [
            MarkdownSyncAnchor(id: "a", kind: .heading, startLine: 1, endLine: 1),
            MarkdownSyncAnchor(id: "b", kind: .heading, startLine: 21, endLine: 21),
            MarkdownSyncAnchor(id: "c", kind: .heading, startLine: 41, endLine: 41),
        ]
        let geometries = [
            MarkdownPreviewAnchorGeometry(anchorID: "a", startLine: 1, endLine: 1, top: 0, height: 30),
            MarkdownPreviewAnchorGeometry(anchorID: "b", startLine: 21, endLine: 21, top: 400, height: 30),
            MarkdownPreviewAnchorGeometry(anchorID: "c", startLine: 41, endLine: 41, top: 800, height: 30),
        ]
        return MarkdownSyncMapBuilder.build(
            MarkdownSyncMapInputs(
                anchors: anchors,
                previewGeometries: geometries,
                editorLineHeight: 20,
                editorMaxScrollY: 800,
                editorViewportHeight: 400,
                previewMaxScrollY: 1000,
                previewViewportHeight: 400
            )
        )
    }
}

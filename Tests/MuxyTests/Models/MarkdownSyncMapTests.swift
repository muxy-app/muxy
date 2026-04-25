import CoreGraphics
import Foundation
import Testing

@testable import Muxy

@Suite("MarkdownSyncMap")
struct MarkdownSyncMapTests {
    @Test("returns zero when empty")
    func emptyMap() {
        let map = MarkdownSyncMap.empty
        #expect(map.isEmpty)
        #expect(map.previewScrollTop(forEditorScrollY: 100) == 0)
        #expect(map.editorScrollY(forPreviewScrollTop: 100) == 0)
    }

    @Test("lerps between adjacent keypoints")
    func lerpBetweenKeypoints() {
        let map = makeMap()

        let target = map.previewScrollTop(forEditorScrollY: 200)
        #expect(abs(target - 200) < 1.0)
    }

    @Test("pins endpoints to (0,0)")
    func pinsHead() {
        let map = makeMap()
        #expect(map.previewScrollTop(forEditorScrollY: -100) == 0)
        #expect(map.editorScrollY(forPreviewScrollTop: -100) == 0)
    }

    @Test("pins endpoints to bottom edge")
    func pinsTail() {
        let map = makeMap()
        let editorMax: CGFloat = 800
        let previewMax: CGFloat = 1000
        #expect(map.previewScrollTop(forEditorScrollY: editorMax) == previewMax)
        #expect(map.editorScrollY(forPreviewScrollTop: previewMax) == editorMax)
    }

    @Test("editor → preview → editor round-trips approximately")
    func roundTrip() {
        let map = makeMap()
        for editorY: CGFloat in [50, 150, 300, 600] {
            let previewY = map.previewScrollTop(forEditorScrollY: editorY)
            let backToEditor = map.editorScrollY(forPreviewScrollTop: previewY)
            #expect(abs(backToEditor - editorY) < 1.0)
        }
    }

    @Test("builder skips anchors without geometry")
    func builderSkipsMissing() {
        let anchors = [
            MarkdownSyncAnchor(id: "a", kind: .heading, startLine: 1, endLine: 1),
            MarkdownSyncAnchor(id: "missing", kind: .heading, startLine: 11, endLine: 11),
            MarkdownSyncAnchor(id: "c", kind: .heading, startLine: 21, endLine: 21),
        ]
        let geometries = [
            MarkdownPreviewAnchorGeometry(anchorID: "a", startLine: 1, endLine: 1, top: 0, height: 30),
            MarkdownPreviewAnchorGeometry(anchorID: "c", startLine: 21, endLine: 21, top: 400, height: 30),
        ]
        let map = MarkdownSyncMapBuilder.build(
            MarkdownSyncMapInputs(
                anchors: anchors,
                previewGeometries: geometries,
                editorLineHeight: 20,
                editorMaxScrollY: 600,
                editorViewportHeight: 400,
                previewMaxScrollY: 800,
                previewViewportHeight: 400
            )
        )

        #expect(!map.isEmpty)
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

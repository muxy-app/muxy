import AppKit
import SwiftUI

struct SplitContainer: View {
    let branch: SplitBranch
    let first: VisiblePaneNode
    let second: VisiblePaneNode
    let topLevelGroupID: UUID
    let focusedAreaID: UUID?
    let isActiveProject: Bool
    let projectID: UUID
    let onSelectPane: (UUID, UUID) -> Void
    let onForceCloseTab: (UUID, UUID) -> Void
    let onDropAction: (TabDragCoordinator.DropResult) -> Void

    var body: some View {
        GeometryReader { geo in
            let horizontal = branch.direction == .horizontal
            let total = horizontal ? geo.size.width : geo.size.height
            let firstLength = max(0, total * branch.ratio - 0.5)
            let secondLength = max(0, total * (1 - branch.ratio) - 0.5)
            let layout = horizontal
                ? AnyLayout(HStackLayout(spacing: 0))
                : AnyLayout(VStackLayout(spacing: 0))

            layout {
                child(first)
                    .frame(
                        width: horizontal ? firstLength : nil,
                        height: horizontal ? nil : firstLength
                    )

                AnchoredResizeHandle(
                    axis: horizontal ? .horizontal : .vertical,
                    captureAnchor: { branch.ratio },
                    onTranslate: { start, delta in
                        guard total > 0 else { return }
                        branch.ratio = min(max(start + delta / total, 0.15), 0.85)
                    }
                )
                .accessibilityLabel(horizontal ? "Horizontal Split Divider" : "Vertical Split Divider")
                .accessibilityValue("Split ratio: \(Int(branch.ratio * 100))%")
                .accessibilityAdjustableAction { direction in
                    let step: CGFloat = 0.05
                    switch direction {
                    case .increment:
                        branch.ratio = min(branch.ratio + step, 0.85)
                    case .decrement:
                        branch.ratio = max(branch.ratio - step, 0.15)
                    @unknown default:
                        break
                    }
                }

                child(second)
                    .frame(
                        width: horizontal ? secondLength : nil,
                        height: horizontal ? nil : secondLength
                    )
            }
        }
    }

    private func child(_ node: VisiblePaneNode) -> some View {
        PaneNode(
            node: node,
            topLevelGroupID: topLevelGroupID,
            focusedAreaID: focusedAreaID,
            isActiveProject: isActiveProject,
            projectID: projectID,
            onSelectPane: onSelectPane,
            onForceCloseTab: onForceCloseTab,
            onDropAction: onDropAction
        )
    }
}

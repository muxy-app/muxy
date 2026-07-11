import AppKit
import SwiftUI

struct SplitContainer: View {
    let branch: SplitBranch
    let focusedAreaID: UUID?
    let isActiveProject: Bool
    let projectID: UUID
    let shortcutOffsets: [UUID: Int]
    let onFocusArea: (UUID) -> Void
    let onSelectTab: (UUID, UUID) -> Void
    let onCreateTab: (UUID) -> Void
    let onCloseTab: (UUID, UUID) -> Void
    let onForceCloseTab: (UUID, UUID) -> Void
    let onSplit: (UUID, SplitDirection) -> Void
    let onCloseArea: (UUID) -> Void
    let onDropAction: (TabDragCoordinator.DropResult) -> Void
    var showMaximizeButton = false
    var onToggleMaximize: ((UUID) -> Void)?

    var body: some View {
        GeometryReader { geo in
            let h = branch.direction == .horizontal
            let total = h ? geo.size.width : geo.size.height
            let dividerInset = UIMetrics.resizeHandleHitArea / 2
            let first = max(0, total * branch.ratio - dividerInset)
            let second = max(0, total * (1 - branch.ratio) - dividerInset)

            let layout = h ? AnyLayout(HStackLayout(spacing: 0)) : AnyLayout(VStackLayout(spacing: 0))

            layout {
                child(branch.first)
                    .frame(width: h ? first : nil, height: h ? nil : first)

                AnchoredResizeHandle(
                    axis: h ? .horizontal : .vertical,
                    captureAnchor: { branch.ratio },
                    onTranslate: { start, delta in
                        guard total > 0 else { return }
                        branch.ratio = min(max(start + delta / total, 0.15), 0.85)
                    }
                )
                .accessibilityLabel(h ? "Horizontal Split Divider" : "Vertical Split Divider")
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

                child(branch.second)
                    .frame(width: h ? second : nil, height: h ? nil : second)
            }
        }
    }

    private func child(_ node: SplitNode) -> some View {
        PaneNode(
            node: node,
            focusedAreaID: focusedAreaID,
            isActiveProject: isActiveProject,
            projectID: projectID,
            shortcutOffsets: shortcutOffsets,
            onFocusArea: onFocusArea,
            onSelectTab: onSelectTab,
            onCreateTab: onCreateTab,
            onCloseTab: onCloseTab,
            onForceCloseTab: onForceCloseTab,
            onSplit: onSplit,
            onCloseArea: onCloseArea,
            onDropAction: onDropAction,
            showMaximizeButton: showMaximizeButton,
            onToggleMaximize: onToggleMaximize
        )
    }
}

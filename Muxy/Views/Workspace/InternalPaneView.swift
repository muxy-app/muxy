import AppKit
import SwiftUI

struct InternalPaneView: View {
    let node: InternalPaneNode
    let focusedPaneID: UUID?
    let areaID: UUID
    let projectID: UUID
    let topLevelGroupID: UUID
    let onFocus: () -> Void
    let onPaneFocus: (UUID) -> Void
    let onProcessExit: () -> Void
    let onSplitRequest: (SplitDirection, SplitPosition) -> Void

    var body: some View {
        switch node {
        case let .pane(pane):
            TerminalPane(
                state: pane,
                focused: focusedPaneID == pane.id,
                visible: true,
                areaID: areaID,
                topLevelGroupID: topLevelGroupID,
                onFocus: { onPaneFocus(pane.id)
                    onFocus()
                },
                onProcessExit: onProcessExit,
                onSplitRequest: onSplitRequest
            )
            .id(pane.id)
        case let .split(branch):
            InternalSplitContainer(
                branch: branch,
                focusedPaneID: focusedPaneID,
                areaID: areaID,
                projectID: projectID,
                topLevelGroupID: topLevelGroupID,
                onFocus: onFocus,
                onPaneFocus: onPaneFocus,
                onProcessExit: onProcessExit,
                onSplitRequest: onSplitRequest
            )
        }
    }
}

private struct InternalSplitContainer: View {
    let branch: InternalBranch
    let focusedPaneID: UUID?
    let areaID: UUID
    let projectID: UUID
    let topLevelGroupID: UUID
    let onFocus: () -> Void
    let onPaneFocus: (UUID) -> Void
    let onProcessExit: () -> Void
    let onSplitRequest: (SplitDirection, SplitPosition) -> Void

    var body: some View {
        GeometryReader { geo in
            let h = branch.direction == .horizontal
            let total = h ? geo.size.width : geo.size.height
            let dividerInset = UIMetrics.resizeHandleHitArea / 2
            let first = max(0, total * branch.ratio - dividerInset)
            let second = max(0, total * (1 - branch.ratio) - dividerInset)

            let layout = h ? AnyLayout(HStackLayout(spacing: 0)) : AnyLayout(VStackLayout(spacing: 0))

            layout {
                InternalPaneView(
                    node: branch.first,
                    focusedPaneID: focusedPaneID,
                    areaID: areaID,
                    projectID: projectID,
                    topLevelGroupID: topLevelGroupID,
                    onFocus: onFocus,
                    onPaneFocus: onPaneFocus,
                    onProcessExit: onProcessExit,
                    onSplitRequest: onSplitRequest
                )
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

                InternalPaneView(
                    node: branch.second,
                    focusedPaneID: focusedPaneID,
                    areaID: areaID,
                    projectID: projectID,
                    topLevelGroupID: topLevelGroupID,
                    onFocus: onFocus,
                    onPaneFocus: onPaneFocus,
                    onProcessExit: onProcessExit,
                    onSplitRequest: onSplitRequest
                )
                .frame(width: h ? second : nil, height: h ? nil : second)
            }
        }
    }
}

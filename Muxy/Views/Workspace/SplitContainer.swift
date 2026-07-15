import AppKit
import SwiftUI

struct SplitContainer: View {
    let branch: SplitBranch
    let focusedAreaID: UUID?
    let isActiveProject: Bool
    let shortcutOffsets: [UUID: Int]
    let actions: WorkspaceViewActions
    var showMaximizeButton = false
    var onToggleMaximize: ((UUID) -> Void)?

    var body: some View {
        GeometryReader { geo in
            let h = branch.direction == .horizontal
            let total = h ? geo.size.width : geo.size.height
            let first = max(0, total * branch.ratio - 0.5)
            let second = max(0, total * (1 - branch.ratio) - 0.5)

            let layout = h ? AnyLayout(HStackLayout(spacing: 0)) : AnyLayout(VStackLayout(spacing: 0))

            layout {
                child(branch.first)
                    .frame(width: h ? first : nil, height: h ? nil : first)

                divider(horizontal: h, total: total)

                child(branch.second)
                    .frame(width: h ? second : nil, height: h ? nil : second)
            }
        }
    }

    @ViewBuilder
    private func divider(horizontal: Bool, total: CGFloat) -> some View {
        if let resizeSplit = actions.resizeSplit {
            AnchoredResizeHandle(
                axis: horizontal ? .horizontal : .vertical,
                captureAnchor: { branch.ratio },
                onTranslate: { start, delta in
                    guard total > 0 else { return }
                    resizeSplit(branch.id, min(max(start + delta / total, 0.15), 0.85))
                }
            )
            .accessibilityLabel(horizontal ? "Horizontal Split Divider" : "Vertical Split Divider")
            .accessibilityValue("Split ratio: \(Int(branch.ratio * 100))%")
            .accessibilityAdjustableAction { direction in
                let step: CGFloat = 0.05
                switch direction {
                case .increment:
                    resizeSplit(branch.id, min(branch.ratio + step, 0.85))
                case .decrement:
                    resizeSplit(branch.id, max(branch.ratio - step, 0.15))
                @unknown default:
                    break
                }
            }
        } else {
            Rectangle()
                .fill(MuxyTheme.border)
                .frame(width: horizontal ? 1 : nil, height: horizontal ? nil : 1)
                .accessibilityHidden(true)
        }
    }

    private func child(_ node: SplitNode) -> some View {
        PaneNode(
            node: node,
            focusedAreaID: focusedAreaID,
            isActiveProject: isActiveProject,
            shortcutOffsets: shortcutOffsets,
            actions: actions,
            showMaximizeButton: showMaximizeButton,
            onToggleMaximize: onToggleMaximize
        )
    }
}

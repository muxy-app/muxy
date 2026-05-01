import AppKit
import SwiftUI

struct SplitContainer: View {
    let branch: SplitBranch
    let focusedAreaID: UUID?
    let isActiveProject: Bool
    let showVCSButton: Bool
    let projectID: UUID
    let shortcutOffsets: [UUID: Int]
    let onFocusArea: (UUID) -> Void
    let onSelectTab: (UUID, UUID) -> Void
    let onCreateTab: (UUID) -> Void
    let onCreateVCSTab: (UUID) -> Void
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
            let first = max(0, total * branch.ratio - 0.5)
            let second = max(0, total * (1 - branch.ratio) - 0.5)

            let layout = h ? AnyLayout(HStackLayout(spacing: 0)) : AnyLayout(VStackLayout(spacing: 0))

            layout {
                child(branch.first)
                    .frame(width: h ? first : nil, height: h ? nil : first)

                Color.clear
                    .frame(width: h ? 1 : nil, height: h ? nil : 1)
                    .overlay(Rectangle().fill(MuxyTheme.border))
                    .overlay {
                        Color.clear
                            .frame(width: h ? UIMetrics.scaled(5) : nil, height: h ? nil : UIMetrics.scaled(5))
                            .contentShape(Rectangle())
                            .gesture(
                                DragGesture(minimumDistance: 1)
                                    .onChanged { v in
                                        let pos = h ? v.location.x : v.location.y
                                        let origin = h ? v.startLocation.x : v.startLocation.y
                                        let startPos = total * branch.ratio
                                        let newPos = startPos + (pos - origin)
                                        branch.ratio = min(max(newPos / total, 0.15), 0.85)
                                    }
                            )
                            .onHover { on in
                                if on { (h ? NSCursor.resizeLeftRight : NSCursor.resizeUpDown).push() } else { NSCursor.pop() }
                            }
                    }
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
            showVCSButton: showVCSButton,
            projectID: projectID,
            shortcutOffsets: shortcutOffsets,
            onFocusArea: onFocusArea,
            onSelectTab: onSelectTab,
            onCreateTab: onCreateTab,
            onCreateVCSTab: onCreateVCSTab,
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

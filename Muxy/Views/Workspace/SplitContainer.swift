import AppKit
import SwiftUI

struct SplitContainer: View {
    let branch: SplitBranch
    let focusedAreaID: UUID?
    let isActiveProject: Bool
    let showVCSButton: Bool
    let projectID: UUID
    let onFocusArea: (UUID) -> Void
    let onSelectTab: (UUID, UUID) -> Void
    let onCreateTab: (UUID) -> Void
    let onCreateVCSTab: (UUID) -> Void
    let onCloseTab: (UUID, UUID) -> Void
    let onForceCloseTab: (UUID, UUID) -> Void
    let onSplit: (UUID, SplitDirection) -> Void
    let onCloseArea: (UUID) -> Void
    let onDropAction: (TabDragCoordinator.DropResult) -> Void

    var body: some View {
        GeometryReader { geo in
            let h = branch.direction == .horizontal
            let total = h ? geo.size.width : geo.size.height

            let layout = h ? AnyLayout(HStackLayout(spacing: 0)) : AnyLayout(VStackLayout(spacing: 0))

            layout {
                ForEach(0 ..< branch.children.count, id: \.self) { index in
                    if index > 0 {
                        divider(index: index, total: total, h: h)
                    }
                    child(branch.children[index])
                        .frame(
                            width: h ? max(0, total * branch.ratios[index] - 0.5) : nil,
                            height: h ? nil : max(0, total * branch.ratios[index] - 0.5)
                        )
                }
            }
        }
    }

    private func divider(index: Int, total: CGFloat, h: Bool) -> some View {
        Color.clear
            .frame(width: h ? 1 : nil, height: h ? nil : 1)
            .overlay(Rectangle().fill(MuxyTheme.border))
            .overlay {
                Color.clear
                    .frame(width: h ? 5 : nil, height: h ? nil : 5)
                    .contentShape(Rectangle())
                    .gesture(
                        DragGesture(minimumDistance: 1)
                            .onChanged { v in
                                let pos = h ? v.location.x : v.location.y
                                let origin = h ? v.startLocation.x : v.startLocation.y

                                let leftIndex = index - 1
                                let rightIndex = index
                                let sharedSpace = branch.ratios[leftIndex] + branch.ratios[rightIndex]

                                let currentBoundary = branch.ratios[0 ... leftIndex].reduce(0, +) * total
                                let newBoundary = currentBoundary + (pos - origin)
                                let newBoundaryRatio = newBoundary / total

                                let leftCumulative = branch.ratios[0 ..< leftIndex].reduce(0, +)
                                var newLeftRatio = newBoundaryRatio - leftCumulative
                                newLeftRatio = min(max(newLeftRatio, 0.15), sharedSpace - 0.15)

                                branch.ratios[leftIndex] = newLeftRatio
                                branch.ratios[rightIndex] = sharedSpace - newLeftRatio
                            }
                    )
                    .onHover { on in
                        if on { (h ? NSCursor.resizeLeftRight : NSCursor.resizeUpDown).push() } else { NSCursor.pop() }
                    }
            }
            .accessibilityLabel(h ? "Horizontal Split Divider" : "Vertical Split Divider")
            .accessibilityValue("Split ratio: \(Int(branch.ratios[index - 1] * 100))%")
            .accessibilityAdjustableAction { direction in
                let step: CGFloat = 0.05
                let leftIndex = index - 1
                let rightIndex = index
                let sharedSpace = branch.ratios[leftIndex] + branch.ratios[rightIndex]
                switch direction {
                case .increment:
                    let newLeft = min(branch.ratios[leftIndex] + step, sharedSpace - 0.15)
                    branch.ratios[rightIndex] = sharedSpace - newLeft
                    branch.ratios[leftIndex] = newLeft
                case .decrement:
                    let newLeft = max(branch.ratios[leftIndex] - step, 0.15)
                    branch.ratios[rightIndex] = sharedSpace - newLeft
                    branch.ratios[leftIndex] = newLeft
                @unknown default:
                    break
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
            onFocusArea: onFocusArea,
            onSelectTab: onSelectTab,
            onCreateTab: onCreateTab,
            onCreateVCSTab: onCreateVCSTab,
            onCloseTab: onCloseTab,
            onForceCloseTab: onForceCloseTab,
            onSplit: onSplit,
            onCloseArea: onCloseArea,
            onDropAction: onDropAction
        )
    }
}

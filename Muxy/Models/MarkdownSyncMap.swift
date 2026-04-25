import CoreGraphics
import Foundation

struct MarkdownSyncKeypoint: Equatable {
    let editorY: CGFloat
    let previewY: CGFloat
}

struct MarkdownSyncMap: Equatable {
    let keypoints: [MarkdownSyncKeypoint]
    let editorMaxScrollY: CGFloat
    let previewMaxScrollY: CGFloat
    let editorViewportHeight: CGFloat
    let previewViewportHeight: CGFloat

    static let empty = MarkdownSyncMap(
        keypoints: [],
        editorMaxScrollY: 0,
        previewMaxScrollY: 0,
        editorViewportHeight: 0,
        previewViewportHeight: 0
    )

    var isEmpty: Bool { keypoints.count < 2 }

    func previewScrollTop(forEditorScrollY editorY: CGFloat) -> CGFloat {
        guard !isEmpty else { return 0 }
        if editorY <= 0 { return 0 }
        if editorY >= editorMaxScrollY { return previewMaxScrollY }
        let fraction = sourceFraction(editorY, isEditor: true)
        let editorCenter = editorY + editorViewportHeight / 2
        let previewCenter = lerp(editorCenter, fromEditor: true)
        let lerpTarget = clamp(previewCenter - previewViewportHeight / 2, lowerBound: 0, upperBound: previewMaxScrollY)
        let edgeTarget = fraction * previewMaxScrollY
        let blend = edgeWeight(forFraction: fraction)
        return lerpTarget * (1 - blend) + edgeTarget * blend
    }

    func editorScrollY(forPreviewScrollTop previewY: CGFloat) -> CGFloat {
        guard !isEmpty else { return 0 }
        if previewY <= 0 { return 0 }
        if previewY >= previewMaxScrollY { return editorMaxScrollY }
        let fraction = sourceFraction(previewY, isEditor: false)
        let previewCenter = previewY + previewViewportHeight / 2
        let editorCenter = lerp(previewCenter, fromEditor: false)
        let lerpTarget = clamp(editorCenter - editorViewportHeight / 2, lowerBound: 0, upperBound: editorMaxScrollY)
        let edgeTarget = fraction * editorMaxScrollY
        let blend = edgeWeight(forFraction: fraction)
        return lerpTarget * (1 - blend) + edgeTarget * blend
    }

    private func sourceFraction(_ value: CGFloat, isEditor: Bool) -> CGFloat {
        let max = isEditor ? editorMaxScrollY : previewMaxScrollY
        guard max > 0 else { return 0 }
        return clamp(value / max, lowerBound: 0, upperBound: 1)
    }

    private func edgeWeight(forFraction fraction: CGFloat) -> CGFloat {
        let edgeBand: CGFloat = 0.05
        if fraction <= edgeBand {
            return 1 - fraction / edgeBand
        }
        if fraction >= 1 - edgeBand {
            return (fraction - (1 - edgeBand)) / edgeBand
        }
        return 0
    }

    private func lerp(_ value: CGFloat, fromEditor: Bool) -> CGFloat {
        let sourceKey: (MarkdownSyncKeypoint) -> CGFloat = fromEditor ? { $0.editorY } : { $0.previewY }
        let targetKey: (MarkdownSyncKeypoint) -> CGFloat = fromEditor ? { $0.previewY } : { $0.editorY }

        if value <= sourceKey(keypoints[0]) {
            return targetKey(keypoints[0])
        }

        for index in 1 ..< keypoints.count {
            let upper = keypoints[index]
            let lower = keypoints[index - 1]
            let upperSource = sourceKey(upper)
            if value <= upperSource {
                let lowerSource = sourceKey(lower)
                let span = upperSource - lowerSource
                if span <= 0.0001 {
                    return targetKey(upper)
                }
                let progress = (value - lowerSource) / span
                let lowerTarget = targetKey(lower)
                let upperTarget = targetKey(upper)
                return lowerTarget + progress * (upperTarget - lowerTarget)
            }
        }

        return targetKey(keypoints[keypoints.count - 1])
    }

    private func clamp(_ value: CGFloat, lowerBound: CGFloat, upperBound: CGFloat) -> CGFloat {
        guard upperBound > lowerBound else { return lowerBound }
        return min(max(value, lowerBound), upperBound)
    }
}

struct MarkdownSyncMapInputs {
    let anchors: [MarkdownSyncAnchor]
    let previewGeometries: [MarkdownPreviewAnchorGeometry]
    let editorLineHeight: CGFloat
    let editorMaxScrollY: CGFloat
    let editorViewportHeight: CGFloat
    let previewMaxScrollY: CGFloat
    let previewViewportHeight: CGFloat
}

enum MarkdownSyncMapBuilder {
    static func build(_ inputs: MarkdownSyncMapInputs) -> MarkdownSyncMap {
        guard inputs.editorLineHeight > 0,
              inputs.editorViewportHeight > 0,
              inputs.previewViewportHeight > 0
        else {
            return .empty
        }

        let editorContentHeight = inputs.editorMaxScrollY + inputs.editorViewportHeight
        let previewContentHeight = inputs.previewMaxScrollY + inputs.previewViewportHeight

        let anchorByID = Dictionary(uniqueKeysWithValues: inputs.anchors.map { ($0.id, $0) })
        var raw: [MarkdownSyncKeypoint] = []
        raw.reserveCapacity(inputs.previewGeometries.count + 2)

        for geometry in inputs.previewGeometries {
            guard let anchor = anchorByID[geometry.anchorID] else { continue }
            let editorY = CGFloat(anchor.startLine - 1) * inputs.editorLineHeight
            raw.append(MarkdownSyncKeypoint(editorY: editorY, previewY: geometry.top))
        }

        raw.sort { $0.editorY < $1.editorY }

        let head = MarkdownSyncKeypoint(editorY: 0, previewY: 0)
        let tail = MarkdownSyncKeypoint(editorY: editorContentHeight, previewY: previewContentHeight)

        var keypoints: [MarkdownSyncKeypoint] = [head]
        for keypoint in raw {
            guard keypoint.editorY > keypoints[keypoints.count - 1].editorY + 0.5,
                  keypoint.previewY > keypoints[keypoints.count - 1].previewY + 0.5
            else { continue }
            keypoints.append(keypoint)
        }
        if tail.editorY > keypoints[keypoints.count - 1].editorY + 0.5,
           tail.previewY > keypoints[keypoints.count - 1].previewY + 0.5
        {
            keypoints.append(tail)
        }

        return MarkdownSyncMap(
            keypoints: keypoints,
            editorMaxScrollY: inputs.editorMaxScrollY,
            previewMaxScrollY: inputs.previewMaxScrollY,
            editorViewportHeight: inputs.editorViewportHeight,
            previewViewportHeight: inputs.previewViewportHeight
        )
    }
}

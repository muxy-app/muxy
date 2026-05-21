import AppKit
import SwiftUI

struct ImageViewerRepresentable: NSViewRepresentable {
    @Bindable var state: ImageViewerTabState

    private static let magnificationEpsilon: CGFloat = 0.0001

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = ZoomableScrollView()
        scrollView.contentView = CenteringClipView()
        scrollView.hasVerticalScroller = false
        scrollView.hasHorizontalScroller = false
        scrollView.allowsMagnification = true
        scrollView.minMagnification = ImageViewerTabState.minScale
        scrollView.maxMagnification = ImageViewerTabState.maxScale
        scrollView.autohidesScrollers = true
        scrollView.scrollerStyle = .overlay
        scrollView.borderType = .noBorder
        scrollView.drawsBackground = false
        scrollView.backgroundColor = .clear
        scrollView.postsFrameChangedNotifications = true
        scrollView.onUserZoom = { [state] newScale in
            guard abs(state.scale - newScale) > Self.magnificationEpsilon else { return }
            state.scale = newScale
        }

        let imageView = NSImageView()
        imageView.imageScaling = .scaleProportionallyUpOrDown
        imageView.imageAlignment = .alignCenter
        imageView.animates = true
        imageView.translatesAutoresizingMaskIntoConstraints = false

        scrollView.documentView = imageView
        context.coordinator.scrollView = scrollView
        context.coordinator.imageView = imageView

        NotificationCenter.default.addObserver(
            context.coordinator,
            selector: #selector(Coordinator.didEndLiveMagnify(_:)),
            name: NSScrollView.didEndLiveMagnifyNotification,
            object: scrollView
        )
        NotificationCenter.default.addObserver(
            context.coordinator,
            selector: #selector(Coordinator.willStartLiveMagnify(_:)),
            name: NSScrollView.willStartLiveMagnifyNotification,
            object: scrollView
        )
        NotificationCenter.default.addObserver(
            context.coordinator,
            selector: #selector(Coordinator.scrollViewFrameDidChange(_:)),
            name: NSView.frameDidChangeNotification,
            object: scrollView
        )

        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        guard let imageView = context.coordinator.imageView else { return }

        if context.coordinator.lastImage !== state.image {
            let isFirstLoad = context.coordinator.lastImage == nil && state.image != nil
            context.coordinator.lastImage = state.image
            imageView.image = state.image
            sizeImageView(imageView)
            if isFirstLoad {
                context.coordinator.needsInitialFit = true
            }
        }

        if context.coordinator.needsInitialFit,
           context.coordinator.fitImage(scrollView: scrollView, imageView: imageView)
        {
            context.coordinator.needsInitialFit = false
        }

        if !context.coordinator.isUserMagnifying,
           abs(scrollView.magnification - state.scale) > Self.magnificationEpsilon
        {
            scrollView.magnification = state.scale
        }

        if context.coordinator.lastFitTrigger != state.fitTrigger,
           context.coordinator.fitImage(scrollView: scrollView, imageView: imageView)
        {
            context.coordinator.lastFitTrigger = state.fitTrigger
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(state: state)
    }

    private func sizeImageView(_ imageView: NSImageView) {
        guard let image = imageView.image else {
            imageView.frame = .zero
            return
        }
        imageView.frame = CGRect(origin: .zero, size: image.size)
    }

    final class ZoomableScrollView: NSScrollView {
        var onUserZoom: ((CGFloat) -> Void)?

        private static let mouseWheelSensitivity: CGFloat = 0.05

        override func scrollWheel(with event: NSEvent) {
            guard !event.hasPreciseScrollingDeltas else {
                super.scrollWheel(with: event)
                return
            }
            let factor = 1.0 + event.scrollingDeltaY * Self.mouseWheelSensitivity
            let proposed = magnification * factor
            let clamped = max(minMagnification, min(maxMagnification, proposed))
            guard abs(clamped - magnification) > ImageViewerRepresentable.magnificationEpsilon else { return }
            let anchor = convert(event.locationInWindow, from: nil)
            setMagnification(clamped, centeredAt: anchor)
            onUserZoom?(clamped)
        }
    }

    final class CenteringClipView: NSClipView {
        override func constrainBoundsRect(_ proposedBounds: NSRect) -> NSRect {
            var rect = super.constrainBoundsRect(proposedBounds)
            guard let documentView else { return rect }
            let docFrame = documentView.frame
            if rect.size.width > docFrame.size.width {
                rect.origin.x = (docFrame.size.width - rect.size.width) / 2
            }
            if rect.size.height > docFrame.size.height {
                rect.origin.y = (docFrame.size.height - rect.size.height) / 2
            }
            return rect
        }
    }

    @MainActor
    final class Coordinator: NSObject {
        weak var scrollView: NSScrollView?
        weak var imageView: NSImageView?
        var lastImage: NSImage?
        var lastFitTrigger: Int = 0
        var needsInitialFit: Bool = false
        var isUserMagnifying: Bool = false
        let state: ImageViewerTabState

        init(state: ImageViewerTabState) {
            self.state = state
        }

        deinit {
            NotificationCenter.default.removeObserver(self)
        }

        @objc
        func willStartLiveMagnify(_ notification: Notification) {
            isUserMagnifying = true
        }

        @objc
        func didEndLiveMagnify(_ notification: Notification) {
            isUserMagnifying = false
            guard let scrollView else { return }
            let newScale = scrollView.magnification
            guard abs(state.scale - newScale) > ImageViewerRepresentable.magnificationEpsilon else { return }
            state.scale = newScale
        }

        @objc
        func scrollViewFrameDidChange(_ notification: Notification) {
            guard needsInitialFit else { return }
            guard let scrollView, let imageView else { return }
            if fitImage(scrollView: scrollView, imageView: imageView) {
                needsInitialFit = false
            }
        }

        @discardableResult
        func fitImage(scrollView: NSScrollView, imageView: NSImageView) -> Bool {
            guard let image = imageView.image, image.size.width > 0, image.size.height > 0 else { return false }
            let bounds = scrollView.bounds.size
            guard bounds.width > 0, bounds.height > 0 else { return false }
            let fit = min(bounds.width / image.size.width, bounds.height / image.size.height, 1.0)
            scrollView.magnification = fit
            if abs(state.scale - fit) > ImageViewerRepresentable.magnificationEpsilon {
                state.scale = fit
            }
            return true
        }
    }
}

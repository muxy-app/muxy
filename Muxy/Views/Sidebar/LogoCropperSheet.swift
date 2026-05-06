import AppKit
import SwiftUI

struct LogoCropperSheet: View {
    let sourceImage: NSImage
    let onConfirm: (NSImage) -> Void
    let onCancel: () -> Void

    @State private var scale: CGFloat = 1.0
    @State private var offset: CGSize = .zero
    @State private var lastOffset: CGSize = .zero

    private let cropSize: CGFloat = 200
    private let outputSize: CGFloat = 128

    private var imageSize: CGSize {
        guard sourceImage.size.width > 0, sourceImage.size.height > 0 else {
            return CGSize(width: cropSize, height: cropSize)
        }
        let aspect = sourceImage.size.width / sourceImage.size.height
        if aspect > 1 {
            return CGSize(width: cropSize * aspect, height: cropSize)
        }
        return CGSize(width: cropSize, height: cropSize / aspect)
    }

    var body: some View {
        VStack(spacing: UIMetrics.spacing7) {
            Text("Crop Logo")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)

            ZStack {
                Color.black

                Image(nsImage: sourceImage)
                    .resizable()
                    .scaledToFill()
                    .frame(
                        width: imageSize.width * scale,
                        height: imageSize.height * scale
                    )
                    .offset(offset)
                    .gesture(dragGesture)
                    .gesture(magnificationGesture)

                RoundedRectangle(cornerRadius: cropSize * (8.0 / 32.0))
                    .stroke(.white.opacity(0.8), lineWidth: 2)
                    .frame(width: cropSize, height: cropSize)
                    .allowsHitTesting(false)

                cropMask
            }
            .frame(width: cropSize + 40, height: cropSize + 40)
            .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusLG))
            .overlay {
                ScrollWheelView { delta in
                    let zoomFactor: CGFloat = 1.0 + delta * 0.03
                    scale = max(0.5, min(5.0, scale * zoomFactor))
                    clampOffset()
                }
                .allowsHitTesting(false)
            }

            HStack(spacing: UIMetrics.spacing6) {
                previewIcon
                VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
                    Text("Preview")
                        .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    Text("Drag to reposition, scroll to zoom")
                        .font(.system(size: UIMetrics.fontCaption))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
                Spacer()
            }

            HStack(spacing: UIMetrics.spacing4) {
                Button("Cancel") { onCancel() }
                    .keyboardShortcut(.cancelAction)
                Spacer()
                Button("Apply") { applyCrop() }
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(300))
        .onAppear { fitImageInitially() }
    }

    private var previewIcon: some View {
        ZStack {
            RoundedRectangle(cornerRadius: UIMetrics.radiusLG)
                .fill(MuxyTheme.surface)
                .frame(width: UIMetrics.scaled(32), height: UIMetrics.scaled(32))

            croppedPreview
                .frame(width: UIMetrics.scaled(32), height: UIMetrics.scaled(32))
                .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusLG))
        }
    }

    private var croppedPreview: some View {
        let previewScale = 32.0 / cropSize
        return Image(nsImage: sourceImage)
            .resizable()
            .scaledToFill()
            .frame(
                width: imageSize.width * scale * previewScale,
                height: imageSize.height * scale * previewScale
            )
            .offset(CGSize(
                width: offset.width * previewScale,
                height: offset.height * previewScale
            ))
    }

    private var cropMask: some View {
        Canvas { context, size in
            let center = CGPoint(x: size.width / 2, y: size.height / 2)
            let cropRect = CGRect(
                x: center.x - cropSize / 2,
                y: center.y - cropSize / 2,
                width: cropSize,
                height: cropSize
            )
            let cornerRadius = cropSize * (8.0 / 32.0)

            context.fill(
                Path(CGRect(origin: .zero, size: size)),
                with: .color(.black.opacity(0.6))
            )
            context.blendMode = .destinationOut
            context.fill(
                Path(roundedRect: cropRect, cornerRadius: cornerRadius),
                with: .color(.white)
            )
        }
        .allowsHitTesting(false)
    }

    private var dragGesture: some Gesture {
        DragGesture()
            .onChanged { value in
                offset = CGSize(
                    width: lastOffset.width + value.translation.width,
                    height: lastOffset.height + value.translation.height
                )
            }
            .onEnded { _ in
                lastOffset = offset
                clampOffset()
            }
    }

    private var magnificationGesture: some Gesture {
        MagnifyGesture()
            .onChanged { value in
                scale = max(0.5, min(5.0, scale * value.magnification))
            }
            .onEnded { _ in
                scale = max(0.5, min(5.0, scale))
                clampOffset()
            }
    }

    private func fitImageInitially() {
        let imgW = sourceImage.size.width
        let imgH = sourceImage.size.height
        guard imgW > 0, imgH > 0 else { return }

        let fitScale = cropSize / min(imageSize.width, imageSize.height)
        scale = max(fitScale, 1.0)
    }

    private func clampOffset() {
        let maxX = max(0, (imageSize.width * scale - cropSize) / 2)
        let maxY = max(0, (imageSize.height * scale - cropSize) / 2)
        offset.width = min(maxX, max(-maxX, offset.width))
        offset.height = min(maxY, max(-maxY, offset.height))
        lastOffset = offset
    }

    private func applyCrop() {
        guard let cgImage = sourceImage.cgImage(
            forProposedRect: nil,
            context: nil,
            hints: nil
        )
        else {
            onCancel()
            return
        }

        let imgW = CGFloat(cgImage.width)
        let imgH = CGFloat(cgImage.height)

        let displayW = imageSize.width * scale
        let displayH = imageSize.height * scale

        let scaleX = imgW / displayW
        let scaleY = imgH / displayH

        let centerX = displayW / 2 - offset.width
        let centerY = displayH / 2 - offset.height

        let cropOriginX = (centerX - cropSize / 2) * scaleX
        let cropOriginY = (centerY - cropSize / 2) * scaleY
        let cropW = cropSize * scaleX
        let cropH = cropSize * scaleY

        let cropRect = CGRect(
            x: max(0, cropOriginX),
            y: max(0, cropOriginY),
            width: min(cropW, imgW - max(0, cropOriginX)),
            height: min(cropH, imgH - max(0, cropOriginY))
        )

        guard let cropped = cgImage.cropping(to: cropRect) else {
            onCancel()
            return
        }

        let finalSize = NSSize(width: outputSize, height: outputSize)
        let result = NSImage(size: finalSize, flipped: false) { drawRect in
            NSGraphicsContext.current?.imageInterpolation = .high
            NSImage(cgImage: cropped, size: .zero).draw(
                in: drawRect,
                from: .zero,
                operation: .copy,
                fraction: 1.0
            )
            return true
        }

        onConfirm(result)
    }
}

private struct ScrollWheelView: NSViewRepresentable {
    let onScroll: (CGFloat) -> Void

    func makeNSView(context: Context) -> ScrollWheelNSView {
        ScrollWheelNSView(onScroll: onScroll)
    }

    func updateNSView(_ nsView: ScrollWheelNSView, context: Context) {
        nsView.onScroll = onScroll
    }
}

private final class ScrollWheelNSView: NSView {
    var onScroll: (CGFloat) -> Void
    private var monitor: Any?

    init(onScroll: @escaping (CGFloat) -> Void) {
        self.onScroll = onScroll
        super.init(frame: .zero)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window != nil, monitor == nil {
            monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] event in
                guard let self,
                      let eventWindow = event.window,
                      eventWindow == self.window
                else { return event }

                let locationInWindow = event.locationInWindow
                let locationInView = self.convert(locationInWindow, from: nil)
                guard self.bounds.contains(locationInView) else { return event }

                let delta = event.scrollingDeltaY
                guard abs(delta) > 0.01 else { return event }
                self.onScroll(delta)
                return nil
            }
        } else if window == nil, let monitor {
            NSEvent.removeMonitor(monitor)
            self.monitor = nil
        }
    }

    override func removeFromSuperview() {
        if let monitor {
            NSEvent.removeMonitor(monitor)
            self.monitor = nil
        }
        super.removeFromSuperview()
    }
}

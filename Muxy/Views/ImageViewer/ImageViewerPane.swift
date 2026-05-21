import SwiftUI

struct ImageViewerPane: View {
    @Bindable var state: ImageViewerTabState
    let focused: Bool
    let onFocus: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            ImageViewerBreadcrumb(state: state)
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            content
        }
        .background(MuxyTheme.bg)
        .contentShape(Rectangle())
        .simultaneousGesture(TapGesture().onEnded { onFocus() })
    }

    @ViewBuilder
    private var content: some View {
        if let errorMessage = state.errorMessage {
            errorView(errorMessage)
        } else if state.isLoaded {
            ImageViewerRepresentable(state: state)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            loadingView
        }
    }

    private var loadingView: some View {
        VStack {
            ProgressView()
                .controlSize(.small)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func errorView(_ message: String) -> some View {
        VStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: UIMetrics.fontTitle))
                .foregroundStyle(MuxyTheme.fgDim)
            Text(message)
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

private struct ImageViewerBreadcrumb: View {
    @Bindable var state: ImageViewerTabState

    var body: some View {
        HStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "photo")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgDim)

            Text(state.filePath)
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)

            if let image = state.image {
                Text("\(Int(image.size.width))×\(Int(image.size.height))")
                    .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgDim)
            }

            if state.scale != 1.0, state.isLoaded {
                Text("\(Int(state.scale * 100))%")
                    .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgDim)
            }

            Spacer()

            IconButton(symbol: "minus.magnifyingglass", size: 12, accessibilityLabel: "Zoom Out") {
                state.zoomOut()
            }
            .help("Zoom Out")
            .disabled(!state.isLoaded || !state.canZoomOut)

            IconButton(symbol: "plus.magnifyingglass", size: 12, accessibilityLabel: "Zoom In") {
                state.zoomIn()
            }
            .help("Zoom In")
            .disabled(!state.isLoaded || !state.canZoomIn)

            IconButton(symbol: "arrow.up.left.and.down.right.magnifyingglass", size: 12, accessibilityLabel: "Fit to Window") {
                state.requestFitToWindow()
            }
            .help("Fit to Window")
            .disabled(!state.isLoaded)

            ImageViewerActualSizeButton {
                state.requestActualSize()
            }
            .disabled(!state.isLoaded)
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .frame(height: UIMetrics.scaled(32))
        .background(MuxyTheme.bg)
    }
}

private struct ImageViewerActualSizeButton: View {
    let action: () -> Void
    @Environment(\.isEnabled) private var isEnabled
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Text("1:1")
                .font(.system(size: UIMetrics.scaled(11), weight: .semibold, design: .monospaced))
                .foregroundStyle(foreground)
                .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .help("Actual Size (100%)")
        .accessibilityLabel("Actual Size")
    }

    private var foreground: Color {
        if !isEnabled { return MuxyTheme.fgDim }
        return hovered ? MuxyTheme.fg : MuxyTheme.fgMuted
    }
}

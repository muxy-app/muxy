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

            IconButton(symbol: "arrow.up.left.and.arrow.down.right.magnifyingglass", size: 11, accessibilityLabel: "Fit to Window") {
                state.requestFitToWindow()
            }
            .help("Fit to Window")
            .disabled(!state.isLoaded)

            IconButton(symbol: "1.magnifyingglass", size: 11, accessibilityLabel: "Actual Size") {
                state.requestActualSize()
            }
            .help("Actual Size (100%)")
            .disabled(!state.isLoaded)
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .frame(height: UIMetrics.scaled(32))
        .background(MuxyTheme.bg)
    }
}

import SwiftUI

struct BrowserToolbar: View {
    let state: BrowserTabState
    @FocusState.Binding var addressFieldFocused: Bool

    @State private var addressText: String = ""

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: UIMetrics.spacing1) {
                IconButton(symbol: "chevron.left", accessibilityLabel: "Back") {
                    state.pendingCommand = .back
                }
                .disabled(!state.canGoBack)
                .opacity(state.canGoBack ? 1 : 0.4)

                IconButton(symbol: "chevron.right", accessibilityLabel: "Forward") {
                    state.pendingCommand = .forward
                }
                .disabled(!state.canGoForward)
                .opacity(state.canGoForward ? 1 : 0.4)

                IconButton(
                    symbol: state.isLoading ? "xmark" : "arrow.clockwise",
                    accessibilityLabel: state.isLoading ? "Stop" : "Reload"
                ) {
                    state.pendingCommand = state.isLoading ? .stop : .reload
                }

                addressField
            }
            .padding(.horizontal, UIMetrics.spacing3)
            .frame(height: UIMetrics.titleBarHeight)

            progressBar
        }
        .background(MuxyTheme.surface)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(MuxyTheme.border)
                .frame(height: 1)
        }
        .onChange(of: state.url) { _, newValue in
            guard !addressFieldFocused else { return }
            addressText = newValue?.absoluteString ?? ""
        }
        .onChange(of: addressFieldFocused) { _, focused in
            guard !focused else { return }
            addressText = state.url?.absoluteString ?? ""
        }
        .onAppear {
            addressText = state.url?.absoluteString ?? ""
        }
    }

    private var addressField: some View {
        TextField("Search or enter address", text: $addressText)
            .textFieldStyle(.plain)
            .font(.system(size: UIMetrics.fontBody))
            .foregroundStyle(MuxyTheme.fg)
            .focused($addressFieldFocused)
            .onSubmit {
                state.load(from: addressText)
                addressFieldFocused = false
            }
            .padding(.horizontal, UIMetrics.spacing4)
            .frame(height: UIMetrics.controlSmall)
            .background(MuxyTheme.bg)
            .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .overlay(
                RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                    .strokeBorder(addressFieldFocused ? MuxyTheme.accent : MuxyTheme.border, lineWidth: 1)
            )
    }

    @ViewBuilder
    private var progressBar: some View {
        if state.isLoading, state.estimatedProgress < 1 {
            GeometryReader { geometry in
                Rectangle()
                    .fill(MuxyTheme.accent)
                    .frame(width: geometry.size.width * state.estimatedProgress, height: 2)
            }
            .frame(height: 2)
        } else {
            Color.clear.frame(height: 2)
        }
    }
}

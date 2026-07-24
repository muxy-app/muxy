import AppKit
import SwiftUI

struct BrowserPane: View {
    let state: BrowserTabState
    let focused: Bool
    let topLevelGroupID: UUID
    let onFocus: () -> Void

    @Environment(AppState.self) private var appState
    @Environment(BrowserHistoryStore.self) private var historyStore
    @Environment(\.overlayActive) private var overlayActive
    @State private var addressFieldFocused = false
    @State private var findVisible = false
    @State private var findFieldFocused = false
    @State private var findQuery = ""

    var body: some View {
        VStack(spacing: 0) {
            BrowserToolbar(
                state: state,
                addressFieldFocused: $addressFieldFocused,
                onAddressFocusClaimed: addressFocusClaimed
            )
            if findVisible {
                BrowserFindBar(
                    state: state,
                    query: $findQuery,
                    fieldFocused: $findFieldFocused,
                    onClose: closeFind
                )
            }
            ZStack {
                BrowserWebView(
                    state: state,
                    focused: Self.shouldFocusWebView(
                        paneFocused: focused,
                        addressFieldFocused: addressFieldFocused,
                        findFieldFocused: findFieldFocused,
                        addressFocusPending: state.shouldFocusAddressOnOpen
                    ),
                    overlayActive: overlayActive,
                    appState: appState,
                    historyStore: historyStore,
                    topLevelGroupID: topLevelGroupID
                )
                .id(state.profileID)
                .contentShape(Rectangle())
                .onTapGesture { onFocus() }

                if let loadError = state.loadError {
                    BrowserErrorView(error: loadError, onRetry: retry)
                } else if state.isBlank, !state.isLoading {
                    BrowserStartPage(
                        searchEngineName: BrowserPreferences.searchEngine.displayName,
                        onFocusAddress: { addressFieldFocused = true }
                    )
                }
            }
        }
        .background(MuxyTheme.bg)
        .background(shortcuts)
        .onAppear(perform: focusAddressFieldOnOpen)
        .onChange(of: focused) { _, isFocused in
            guard isFocused else {
                addressFieldFocused = false
                return
            }
            focusAddressFieldOnOpen()
        }
        .onChange(of: state.findActivationToken) { _, _ in findVisible = true }
    }

    private func closeFind() {
        findVisible = false
        findFieldFocused = false
    }

    private func retry() {
        state.loadError = nil
        state.pendingCommand = .reload
    }

    private func copyCurrentURL() {
        guard let absoluteString = state.url?.absoluteString,
              !BrowserHomePage.isBlankMode(absoluteString)
        else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(absoluteString, forType: .string)
    }

    private func focusAddressFieldOnOpen() {
        guard focused, state.shouldFocusAddressOnOpen else { return }
        addressFieldFocused = true
    }

    private func addressFocusClaimed() {
        state.shouldFocusAddressOnOpen = false
    }

    static func shouldFocusWebView(
        paneFocused: Bool,
        addressFieldFocused: Bool,
        findFieldFocused: Bool,
        addressFocusPending: Bool
    ) -> Bool {
        paneFocused && !addressFieldFocused && !findFieldFocused && !addressFocusPending
    }

    @ViewBuilder
    private var shortcuts: some View {
        if focused {
            Group {
                Button("") { addressFieldFocused = true }
                    .keyboardShortcut("l", modifiers: .command)
                Button("") { state.pendingCommand = .reload }
                    .keyboardShortcut("r", modifiers: .command)
                Button("") { state.pendingCommand = .back }
                    .keyboardShortcut("[", modifiers: .command)
                Button("") { state.pendingCommand = .forward }
                    .keyboardShortcut("]", modifiers: .command)
                Button("") { state.pendingCommand = .zoomIn }
                    .keyboardShortcut("=", modifiers: .command)
                Button("") { state.pendingCommand = .zoomOut }
                    .keyboardShortcut("-", modifiers: .command)
                Button("") { state.pendingCommand = .zoomReset }
                    .keyboardShortcut("0", modifiers: .command)
                Button("", action: copyCurrentURL)
                    .keyboardShortcut("c", modifiers: [.command, .shift])
            }
            .opacity(0)
            .frame(width: 0, height: 0)
            .accessibilityHidden(true)
        }
    }
}

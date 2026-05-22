import SwiftUI

struct BrowserPane: View {
    @Bindable var state: BrowserTabState
    let focused: Bool
    let onFocus: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            BrowserChrome(state: state)
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            BrowserProgressBar(progress: state.estimatedProgress, isLoading: state.isLoading)
            HStack(spacing: 0) {
                content
                if state.showsAnnotationsPanel {
                    Rectangle().fill(MuxyTheme.border).frame(width: 1)
                    BrowserAnnotationsPanel(state: state)
                        .frame(width: UIMetrics.scaled(280))
                }
            }
        }
        .background(MuxyTheme.bg)
        .contentShape(Rectangle())
        .simultaneousGesture(TapGesture().onEnded { onFocus() })
    }

    @ViewBuilder
    private var content: some View {
        if let error = state.lastErrorMessage {
            errorView(error)
        } else {
            BrowserWebView(state: state)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private func errorView(_ message: String) -> some View {
        VStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: UIMetrics.fontTitle))
                .foregroundStyle(MuxyTheme.fgDim)
            Text(message)
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
            Button("Retry") { state.requestReload() }
                .buttonStyle(.plain)
                .padding(.horizontal, UIMetrics.spacing4)
                .padding(.vertical, UIMetrics.spacing2)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
                .foregroundStyle(MuxyTheme.fg)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

private struct BrowserChrome: View {
    @Bindable var state: BrowserTabState
    @Environment(AppState.self) private var appState
    @State private var addressFieldText: String = ""
    @FocusState private var addressFieldFocused: Bool

    var body: some View {
        HStack(spacing: UIMetrics.spacing3) {
            IconButton(symbol: "chevron.left", accessibilityLabel: "Back") { state.requestBack() }
                .help("Back")
                .disabled(!state.canGoBack)
            IconButton(symbol: "chevron.right", accessibilityLabel: "Forward") { state.requestForward() }
                .help("Forward")
                .disabled(!state.canGoForward)
            IconButton(
                symbol: state.isLoading ? "xmark" : "arrow.clockwise",
                accessibilityLabel: state.isLoading ? "Stop" : "Reload"
            ) {
                if state.isLoading {
                    state.requestStop()
                } else {
                    state.requestReload()
                }
            }
            .help(state.isLoading ? "Stop" : "Reload")

            BrowserAddressField(
                text: $addressFieldText,
                isLoading: state.isLoading,
                scheme: state.currentURLScheme,
                isFocused: $addressFieldFocused,
                onSubmit: {
                    state.requestNavigate(to: addressFieldText)
                    addressFieldFocused = false
                }
            )

            BookmarkMenu(state: state)

            BrowserInspectorToggle(state: state)

            Menu {
                Button("Zoom In") { state.zoomIn() }
                    .keyboardShortcut("=", modifiers: [.command])
                Button("Zoom Out") { state.zoomOut() }
                    .keyboardShortcut("-", modifiers: [.command])
                Button("Actual Size") { state.resetZoom() }
                    .keyboardShortcut("0", modifiers: [.command])
                Divider()
                Button("Open in System Browser") {
                    if let url = URL(string: state.currentURL) {
                        NSWorkspace.shared.open(url)
                    }
                }
            } label: {
                Image(systemName: "ellipsis.circle")
                    .font(.system(size: UIMetrics.scaled(13), weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
            }
            .menuStyle(.button)
            .buttonStyle(.plain)
            .menuIndicator(.hidden)
            .fixedSize()
            .help("More")
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .frame(height: UIMetrics.scaled(36))
        .background(MuxyTheme.bg)
        .onAppear { addressFieldText = state.currentURL }
        .onChange(of: state.currentURL) { _, newValue in
            if !addressFieldFocused {
                addressFieldText = newValue
            }
        }
    }
}

private struct BrowserAddressField: View {
    @Binding var text: String
    let isLoading: Bool
    let scheme: String?
    var isFocused: FocusState<Bool>.Binding
    let onSubmit: () -> Void

    var body: some View {
        HStack(spacing: UIMetrics.spacing2) {
            if isLoading {
                ProgressView()
                    .controlSize(.mini)
            } else {
                Image(systemName: securityIcon)
                    .font(.system(size: UIMetrics.fontCaption))
                    .foregroundStyle(securityIconTint)
                    .help(securityIconHelp)
            }
            TextField("URL or search", text: $text)
                .textFieldStyle(.plain)
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fg)
                .focused(isFocused)
                .onSubmit(onSubmit)
        }
        .padding(.horizontal, UIMetrics.spacing4)
        .frame(height: UIMetrics.scaled(26))
        .frame(maxWidth: .infinity)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .overlay(
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                .strokeBorder(isFocused.wrappedValue ? MuxyTheme.accent : MuxyTheme.border, lineWidth: 1)
        )
    }

    private var securityIcon: String {
        switch scheme {
        case "https": "lock.fill"
        case "http": "lock.open"
        default: "globe"
        }
    }

    private var securityIconTint: Color {
        scheme == "https" ? MuxyTheme.fgDim : MuxyTheme.warning
    }

    private var securityIconHelp: String {
        switch scheme {
        case "https": "Secure connection"
        case "http": "Connection is not secure"
        default: "Connection status unknown"
        }
    }
}

private struct BrowserProgressBar: View {
    let progress: Double
    let isLoading: Bool

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Color.clear
                if isLoading {
                    Rectangle()
                        .fill(MuxyTheme.accent)
                        .frame(width: max(0, geo.size.width * progress))
                        .animation(.easeInOut(duration: 0.2), value: progress)
                }
            }
        }
        .frame(height: 2)
    }
}

private struct BrowserInspectorToggle: View {
    @Bindable var state: BrowserTabState

    var body: some View {
        HStack(spacing: 0) {
            inspectorButton(
                mode: .annotate,
                symbol: "bubble.left.and.bubble.right",
                title: "Annotate"
            )
            inspectorButton(
                mode: .style,
                symbol: "paintbrush",
                title: "Style"
            )
        }
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
    }

    private func inspectorButton(mode: BrowserTabState.InspectorMode, symbol: String, title: String) -> some View {
        Button {
            state.setInspectorMode(state.inspectorMode == mode ? .off : mode)
        } label: {
            Image(systemName: symbol)
                .font(.system(size: UIMetrics.scaled(12), weight: .semibold))
                .foregroundStyle(state.inspectorMode == mode ? MuxyTheme.accent : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(state.inspectorMode == mode ? "Disable \(title)" : "Enable \(title)")
        .accessibilityLabel(title)
    }
}

private struct BookmarkMenu: View {
    @Bindable var state: BrowserTabState
    private let bookmarkStore = BrowserBookmarkStore.shared

    var body: some View {
        Menu {
            Button {
                let bookmark = BrowserBookmark(
                    title: state.pageTitle.isEmpty ? state.currentURL : state.pageTitle,
                    url: state.currentURL
                )
                bookmarkStore.add(bookmark, projectPath: state.projectPath)
            } label: {
                Label("Bookmark This Page", systemImage: "bookmark.fill")
            }
            .disabled(state.currentURL.isEmpty || state.currentURL == BrowserTabState.defaultURL)

            Divider()

            let bookmarks = bookmarkStore.bookmarks(for: state.projectPath)
            if bookmarks.isEmpty {
                Button("No Bookmarks") {}
                    .disabled(true)
            } else {
                ForEach(bookmarks) { bookmark in
                    Button {
                        state.requestNavigate(to: bookmark.url)
                    } label: {
                        VStack(alignment: .leading) {
                            Text(bookmark.title)
                            Text(bookmark.url)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                Divider()
                Menu("Remove Bookmark") {
                    ForEach(bookmarks) { bookmark in
                        Button(bookmark.title) {
                            bookmarkStore.remove(id: bookmark.id, projectPath: state.projectPath)
                        }
                    }
                }
            }
        } label: {
            Image(systemName: "bookmark")
                .font(.system(size: UIMetrics.scaled(12), weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .help("Bookmarks")
    }
}

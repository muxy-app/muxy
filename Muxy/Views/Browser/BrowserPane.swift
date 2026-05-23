import SwiftUI

struct BrowserPane: View {
    @Bindable var session: BrowserSession
    let focused: Bool
    let onFocus: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            BrowserChrome(session: session)
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            BrowserProgressBar(progress: session.nav.estimatedProgress, isLoading: session.nav.isLoading)
            HStack(spacing: 0) {
                content
                if session.inspector.showsAnnotationsPanel {
                    Rectangle().fill(MuxyTheme.border).frame(width: 1)
                    BrowserAnnotationsPanel(session: session)
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
        if let error = session.nav.lastErrorMessage {
            errorView(error)
        } else {
            ZStack(alignment: .top) {
                BrowserWebView(session: session)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                if session.nav.findBar.isVisible {
                    BrowserFindBar(session: session)
                        .padding(UIMetrics.spacing3)
                }
            }
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
            Button("Retry") { session.requestReload() }
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
    @Bindable var session: BrowserSession
    @Environment(AppState.self) private var appState
    @State private var addressFieldText: String = ""
    @FocusState private var addressFieldFocused: Bool

    var body: some View {
        HStack(spacing: UIMetrics.spacing3) {
            IconButton(symbol: "chevron.left", accessibilityLabel: "Back") { session.requestBack() }
                .help("Back")
                .disabled(!session.nav.canGoBack)
            IconButton(symbol: "chevron.right", accessibilityLabel: "Forward") { session.requestForward() }
                .help("Forward")
                .disabled(!session.nav.canGoForward)
            IconButton(
                symbol: session.nav.isLoading ? "xmark" : "arrow.clockwise",
                accessibilityLabel: session.nav.isLoading ? "Stop" : "Reload"
            ) {
                if session.nav.isLoading {
                    session.requestStop()
                } else {
                    session.requestReload()
                }
            }
            .help(session.nav.isLoading ? "Stop" : "Reload")

            BrowserAddressField(
                text: $addressFieldText,
                isLoading: session.nav.isLoading,
                scheme: session.nav.currentURLScheme,
                isFocused: $addressFieldFocused,
                onSubmit: {
                    session.requestNavigate(to: addressFieldText)
                    addressFieldFocused = false
                }
            )

            BookmarkMenu(session: session)

            BrowserInspectorToggle(session: session)

            Menu {
                Button("Zoom In") { session.zoomIn() }
                    .keyboardShortcut("=", modifiers: [.command])
                Button("Zoom Out") { session.zoomOut() }
                    .keyboardShortcut("-", modifiers: [.command])
                Button("Actual Size") { session.resetZoom() }
                    .keyboardShortcut("0", modifiers: [.command])
                Divider()
                Button("Open in System Browser") {
                    if let url = URL(string: session.nav.currentURL) {
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
        .onAppear { addressFieldText = session.nav.currentURL }
        .onChange(of: session.nav.currentURL) { _, newValue in
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
    @Bindable var session: BrowserSession

    private var isActive: Bool { session.inspector.inspectorMode == .pick }

    var body: some View {
        Button {
            session.setInspectorMode(isActive ? .off : .pick)
        } label: {
            Image(systemName: "cursorarrow.click")
                .font(.system(size: UIMetrics.scaled(12), weight: .semibold))
                .foregroundStyle(isActive ? MuxyTheme.accent : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
                .contentShape(Rectangle())
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        }
        .buttonStyle(.plain)
        .help(isActive ? "Stop inspecting" : "Inspect an element to comment or restyle")
        .accessibilityLabel("Inspect")
        .accessibilityHint("Click an element on the page to add a comment or edit its styles")
        .accessibilityAddTraits(isActive ? [.isSelected] : [])
    }
}

private struct BookmarkMenu: View {
    @Bindable var session: BrowserSession
    private let bookmarkStore = BrowserBookmarkStore.shared

    var body: some View {
        Menu {
            Button {
                let bookmark = BrowserBookmark(
                    title: session.nav.pageTitle.isEmpty ? session.nav.currentURL : session.nav.pageTitle,
                    url: session.nav.currentURL
                )
                bookmarkStore.add(bookmark, projectPath: session.projectPath)
            } label: {
                Label("Bookmark This Page", systemImage: "bookmark.fill")
            }
            .disabled(session.nav.currentURL.isEmpty || session.nav.currentURL == BrowserSession.defaultURL)

            Divider()

            let bookmarks = bookmarkStore.bookmarks(for: session.projectPath)
            if bookmarks.isEmpty {
                Button("No Bookmarks") {}
                    .disabled(true)
            } else {
                ForEach(bookmarks) { bookmark in
                    Button {
                        session.requestNavigate(to: bookmark.url)
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
                            bookmarkStore.remove(id: bookmark.id, projectPath: session.projectPath)
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

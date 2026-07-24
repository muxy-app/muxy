import AppKit
import SwiftUI

struct BrowserToolbar: View {
    let state: BrowserTabState
    @Binding var addressFieldFocused: Bool
    let onAddressFocusClaimed: () -> Void

    @Environment(BrowserProfileStore.self) private var profileStore
    @Environment(BrowserHistoryStore.self) private var historyStore
    @State private var addressText: String = ""
    @State private var installedBrowsers: [InstalledBrowser] = []
    @State private var suggestionModel = BrowserSuggestionModel()

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

                openInBrowserMenu

                profilePicker
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
        .onChange(of: state.url) { _, _ in
            guard !addressFieldFocused else { return }
            addressText = displayURLText
        }
        .onChange(of: addressFieldFocused) { _, focused in
            guard !focused else { return }
            addressText = displayURLText
        }
        .onAppear {
            addressText = displayURLText
        }
    }

    private func submitAddress(_ selected: BrowserHistoryEntry?, text: String) {
        let target = selected?.url ?? text
        addressText = target
        state.load(from: target)
        addressFieldFocused = false
    }

    private var openInBrowserMenu: some View {
        Menu {
            Button("Open in Default Browser") { openInDefaultBrowser() }
            if !installedBrowsers.isEmpty {
                Divider()
                ForEach(installedBrowsers) { browser in
                    Button(browser.name) { openURL(in: browser) }
                }
            }
        } label: {
            Image(systemName: "arrow.up.forward.app")
                .font(.system(size: UIMetrics.fontBody, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
                .contentShape(Rectangle())
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fixedSize()
        .disabled(state.url == nil)
        .help("Open in External Browser")
        .onAppear {
            if installedBrowsers.isEmpty {
                installedBrowsers = InstalledBrowsers.all()
            }
        }
    }

    private func openInDefaultBrowser() {
        guard let url = state.url else { return }
        NSWorkspace.shared.open(url)
    }

    private func openURL(in browser: InstalledBrowser) {
        guard let url = state.url else { return }
        InstalledBrowsers.open(url, in: browser)
    }

    private var profilePicker: some View {
        Menu {
            ForEach(profileStore.profiles) { profile in
                Button {
                    selectProfile(profile.id)
                } label: {
                    if profile.id == state.profileID {
                        Label(profile.name, systemImage: "checkmark")
                    } else {
                        Text(profile.name)
                    }
                }
            }
            Divider()
            Button("Manage Profiles…") {
                SettingsFocusCoordinator.shared.request(.browser)
                NotificationCenter.default.post(name: .openSettingsModal, object: nil)
            }
        } label: {
            HStack(spacing: UIMetrics.spacing1) {
                Image(systemName: "person.crop.circle")
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                Text(currentProfileName)
                    .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                    .lineLimit(1)
            }
            .foregroundStyle(MuxyTheme.fgMuted)
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fixedSize()
        .help("Browser Profile")
    }

    private var currentProfileName: String {
        profileStore.profile(id: state.profileID)?.name ?? BrowserProfile.defaultName
    }

    private func selectProfile(_ id: UUID) {
        state.switchProfile(to: id)
    }

    private var addressField: some View {
        BrowserAddressField(
            text: $addressText,
            isFocused: $addressFieldFocused,
            model: suggestionModel,
            suggestionsProvider: { historyStore.suggestions(for: $0, profileID: state.profileID) },
            onFocusClaimed: onAddressFocusClaimed,
            onSubmit: submitAddress
        )
        .padding(.horizontal, UIMetrics.spacing4)
        .frame(height: UIMetrics.controlSmall)
        .background(MuxyTheme.bg)
        .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .overlay(
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                .strokeBorder(addressFieldFocused ? MuxyTheme.accent : MuxyTheme.border, lineWidth: 1)
        )
    }

    private var displayURLText: String {
        guard let absoluteString = state.url?.absoluteString,
              !BrowserHomePage.isBlankMode(absoluteString)
        else { return "" }
        return absoluteString
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

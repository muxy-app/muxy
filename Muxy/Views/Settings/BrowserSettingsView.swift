import SwiftUI

struct BrowserSettingsView: View {
    @Environment(BrowserProfileStore.self) private var profileStore
    @Environment(BrowserHistoryStore.self) private var historyStore
    @AppStorage(BrowserPreferences.enabledKey) private var browserEnabled = true
    @AppStorage(BrowserPreferences.openLinksInBuiltInBrowserKey) private var openLinksInBuiltInBrowser = false
    @AppStorage(BrowserPreferences.searchEngineKey) private var searchEngineRawValue = BrowserPreferences
        .defaultSearchEngine.rawValue
    @AppStorage(BrowserPreferences.homePageURLKey) private var homePageURLString = BrowserHomePage.blankURLString

    @State private var editorMode: BrowserProfileEditorMode?
    @State private var profilePendingDelete: BrowserProfile?
    @State private var profilePendingClear: BrowserProfile?
    @State private var importTarget: BrowserProfile?
    @State private var usesCustomHomePage = false
    @State private var customHomePageDraft = ""

    private static let profilesFooter = """
    Each profile keeps its own cookies, cache, and logins. Pick a profile per tab from the browser \
    toolbar. Import brings an existing browser's cookies so tabs start signed in.
    """

    private static let disabledFooter = """
    The built-in browser is off. Browser tabs, the toolbar globe, and terminal-link opening are \
    disabled, and terminal links open in your system browser.
    """

    private static let homePageFooter = """
    New browser tabs open to a blank page. Turn on the toggle to open them to a website instead.
    """

    var body: some View {
        SettingsContainer {
            SettingsSection("General", footer: browserEnabled ? nil : Self.disabledFooter, showsDivider: browserEnabled) {
                SettingsToggleRow(
                    label: "Enable Built-in Browser",
                    isOn: $browserEnabled
                )
                if browserEnabled {
                    SettingsToggleRow(
                        label: "Open terminal links in built-in browser",
                        isOn: $openLinksInBuiltInBrowser
                    )
                    SettingsRow("Default Profile") {
                        Picker("", selection: defaultProfileBinding) {
                            ForEach(profileStore.profiles) { profile in
                                Text(profile.name).tag(profile.id)
                            }
                        }
                        .labelsHidden()
                        .frame(width: SettingsMetrics.controlWidth, alignment: .trailing)
                    }
                }
            }

            if browserEnabled {
                SettingsSection("Browsing", footer: Self.homePageFooter) {
                    SettingsRow("Search Engine") {
                        Picker("", selection: $searchEngineRawValue) {
                            ForEach(BrowserSearchEngine.allCases) { engine in
                                Text(engine.displayName).tag(engine.rawValue)
                            }
                        }
                        .labelsHidden()
                        .frame(width: SettingsMetrics.controlWidth, alignment: .trailing)
                    }
                    SettingsToggleRow(label: "Open new tabs to a website", isOn: customHomePageEnabledBinding)
                    if usesCustomHomePage {
                        SettingsRow("Home Page") {
                            TextField("https://example.com", text: $customHomePageDraft)
                                .settingsTextInput(width: SettingsMetrics.controlWidth)
                                .onSubmit { commitCustomHomePage() }
                                .onChange(of: customHomePageDraft) { _, _ in commitCustomHomePage() }
                        }
                    }
                }
            }

            if browserEnabled {
                SettingsSection("Profiles", footer: Self.profilesFooter, showsDivider: false) {
                    ForEach(profileStore.profiles) { profile in
                        BrowserProfileRow(
                            profile: profile,
                            onRename: { editorMode = .edit(profile) },
                            onImport: { importTarget = profile },
                            onClearData: { profilePendingClear = profile },
                            onDelete: { profilePendingDelete = profile }
                        )
                    }
                    addButton
                }
            }
        }
        .onAppear { syncHomePageDraft() }
        .sheet(item: $editorMode) { mode in
            BrowserProfileEditorSheet(
                mode: mode,
                onSave: { name in
                    save(mode: mode, name: name)
                    editorMode = nil
                },
                onCancel: { editorMode = nil }
            )
        }
        .sheet(item: $importTarget) { profile in
            BrowserImportSheet(
                targetProfile: profile,
                onDismiss: { importTarget = nil }
            )
        }
        .alert(
            "Delete “\(profilePendingDelete?.name ?? "")”?",
            isPresented: deleteAlertBinding,
            presenting: profilePendingDelete
        ) { profile in
            Button("Delete", role: .destructive) {
                historyStore.clear(profileID: profile.id)
                profileStore.remove(id: profile.id)
                profilePendingDelete = nil
            }
            .keyboardShortcut(.defaultAction)
            Button("Cancel", role: .cancel) { profilePendingDelete = nil }
        } message: { _ in
            Text("This permanently deletes the profile's cookies and browsing data.")
        }
        .alert(
            "Clear data for “\(profilePendingClear?.name ?? "")”?",
            isPresented: clearAlertBinding,
            presenting: profilePendingClear
        ) { profile in
            Button("Clear Data", role: .destructive) {
                let id = profile.id
                let name = profile.name
                profilePendingClear = nil
                historyStore.clear(profileID: id)
                Task {
                    await profileStore.clearData(for: id)
                    ToastState.shared.show("Cleared browsing data for “\(name)”")
                }
            }
            .keyboardShortcut(.defaultAction)
            Button("Cancel", role: .cancel) { profilePendingClear = nil }
        } message: { _ in
            Text("This signs out and removes all cookies, cache, and logins for this profile, including imported ones.")
        }
    }

    private var addButton: some View {
        Button {
            editorMode = .create
        } label: {
            HStack(spacing: 6) {
                Image(systemName: "plus")
                    .font(.system(size: 11, weight: .semibold))
                Text("Add Profile")
                    .font(.system(size: SettingsMetrics.labelFontSize, weight: .medium))
            }
            .foregroundStyle(SettingsStyle.accent)
            .padding(.horizontal, SettingsMetrics.horizontalPadding)
            .padding(.vertical, SettingsMetrics.rowVerticalPadding)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private var defaultProfileBinding: Binding<UUID> {
        Binding(
            get: { profileStore.defaultProfileID },
            set: { profileStore.setDefault(id: $0) }
        )
    }

    private var customHomePageEnabledBinding: Binding<Bool> {
        Binding(
            get: { usesCustomHomePage },
            set: { enabled in
                usesCustomHomePage = enabled
                homePageURLString = enabled ? BrowserHomePage.normalized(customHomePageDraft) : BrowserHomePage
                    .blankURLString
            }
        )
    }

    private func commitCustomHomePage() {
        homePageURLString = BrowserHomePage.normalized(customHomePageDraft)
    }

    private func syncHomePageDraft() {
        usesCustomHomePage = !BrowserHomePage.isBlankMode(homePageURLString)
        customHomePageDraft = usesCustomHomePage ? homePageURLString : ""
    }

    private var deleteAlertBinding: Binding<Bool> {
        Binding(
            get: { profilePendingDelete != nil },
            set: { if !$0 { profilePendingDelete = nil } }
        )
    }

    private var clearAlertBinding: Binding<Bool> {
        Binding(
            get: { profilePendingClear != nil },
            set: { if !$0 { profilePendingClear = nil } }
        )
    }

    private func save(mode: BrowserProfileEditorMode, name: String) {
        switch mode {
        case .create:
            profileStore.add(name: name)
        case let .edit(profile):
            profileStore.rename(id: profile.id, to: name)
        }
    }
}

private struct BrowserProfileRow: View {
    let profile: BrowserProfile
    let onRename: () -> Void
    let onImport: () -> Void
    let onClearData: () -> Void
    let onDelete: () -> Void

    @State private var isHovered = false

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "person.crop.circle")
                .font(.system(size: SettingsMetrics.labelFontSize))
                .foregroundStyle(SettingsStyle.mutedForeground)
                .frame(width: 16)
            Text(profile.name)
                .font(.system(size: SettingsMetrics.labelFontSize, weight: .medium))
                .foregroundStyle(SettingsStyle.foreground)
            if profile.isDefault {
                defaultBadge
            }
            Spacer()
            actionsMenu
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
        .background(
            isHovered ? SettingsStyle.hover : .clear,
            in: RoundedRectangle(cornerRadius: 6)
        )
        .onHover { isHovered = $0 }
    }

    private var defaultBadge: some View {
        Text("Default")
            .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium))
            .foregroundStyle(SettingsStyle.mutedForeground)
            .padding(.horizontal, 6)
            .padding(.vertical, 1)
            .background(SettingsStyle.hover, in: Capsule())
    }

    private var actionsMenu: some View {
        Menu {
            Button("Import from Chrome…", action: onImport)
            if !profile.isDefault {
                Button("Rename…", action: onRename)
            }
            Divider()
            Button("Clear Data", role: .destructive, action: onClearData)
            if !profile.isDefault {
                Button("Delete", role: .destructive, action: onDelete)
            }
        } label: {
            Image(systemName: "ellipsis")
                .font(.system(size: SettingsMetrics.labelFontSize, weight: .semibold))
                .foregroundStyle(SettingsStyle.mutedForeground)
                .frame(height: 20)
                .contentShape(Rectangle())
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fixedSize()
    }
}

enum BrowserProfileEditorMode: Identifiable {
    case create
    case edit(BrowserProfile)

    var id: String {
        switch self {
        case .create: "profile-create"
        case let .edit(profile): "profile-edit-\(profile.id.uuidString)"
        }
    }

    var title: String {
        switch self {
        case .create: "Add Profile"
        case .edit: "Rename Profile"
        }
    }

    var initialName: String {
        switch self {
        case .create: ""
        case let .edit(profile): profile.name
        }
    }
}

private struct BrowserProfileEditorSheet: View {
    let mode: BrowserProfileEditorMode
    let onSave: (String) -> Void
    let onCancel: () -> Void

    @State private var name = ""

    private var canSave: Bool {
        !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text(mode.title)
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            TextField("Profile name", text: $name)
                .settingsTextInput(maxWidth: .infinity)

            HStack(spacing: UIMetrics.spacing3) {
                Spacer()
                Button("Cancel", action: onCancel)
                    .keyboardShortcut(.cancelAction)
                Button("Save") { onSave(name) }
                    .keyboardShortcut(.defaultAction)
                    .disabled(!canSave)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(360))
        .onAppear { name = mode.initialName }
    }
}

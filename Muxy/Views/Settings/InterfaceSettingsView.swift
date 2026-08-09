import SwiftUI

struct InterfaceSettingsView: View {
    @State private var uiScale = UIScale.shared
    @State private var themeService = ThemeService.shared
    @State private var extensionStore = ExtensionStore.shared
    @State private var localizationService = LocalizationService.shared
    @State private var showLightThemePicker = false
    @State private var showDarkThemePicker = false
    @State private var currentLightTheme: String?
    @State private var currentDarkTheme: String?
    @AppStorage(AppBackgroundStyle.storageKey)
    private var appBackgroundStyleRaw = AppBackgroundStyle.defaultValue.rawValue
    @AppStorage(AppTransparencyPreferences.transparencyKey)
    private var appTransparency = AppTransparencyPreferences.defaultTransparency
    @AppStorage(AppTransparencyPreferences.blurIntensityKey)
    private var appBlurIntensity = AppTransparencyPreferences.defaultBlurIntensity
    @AppStorage("muxy.showStatusBar") private var showStatusBar = true
    @AppStorage(ResourceUsagePreferences.visibleKey) private var showResourceUsage = ResourceUsagePreferences.defaultVisible
    @State private var layoutStore = AppLayoutStore.shared
    @AppStorage(GeneralSettingsKeys.autoExpandWorktreesOnProjectSwitch) private var autoExpandWorktrees = false
    @AppStorage(SidebarCollapsedStyle.storageKey) private var sidebarCollapsedStyle = SidebarCollapsedStyle.defaultValue.rawValue
    @AppStorage(SidebarExpandedStyle.storageKey) private var sidebarExpandedStyle = SidebarExpandedStyle.defaultValue.rawValue
    @AppStorage(HomeProjectPreferences.visibleKey) private var showHomeProject = HomeProjectPreferences.defaultVisible
    @AppStorage(TipsPreferences.visibleKey) private var showTips = TipsPreferences.defaultVisible
    @AppStorage(SidebarSelection.storageKey) private var activeSidebar = SidebarSelection.builtinValue
    @AppStorage(LocalizationSelection.storageKey)
    private var selectedLocalization = LocalizationSelection.builtinValue
    @AppStorage(WorktreeListPreferences.showUnreadIndicatorKey)
    private var showWorktreeUnreadIndicator = WorktreeListPreferences.defaultShowUnreadIndicator
    @AppStorage(WorktreeListPreferences.orderByMRUKey)
    private var orderWorktreesByMRU = WorktreeListPreferences.defaultOrderByMRU
    @AppStorage(WorktreeListPreferences.groupWorktreesKey)
    private var groupWorktrees = WorktreeListPreferences.defaultGroupWorktrees
    @AppStorage(ProjectSearchPreferences.visibleKey)
    private var showProjectSearch = ProjectSearchPreferences.defaultVisible

    private var layoutSelection: Binding<AppLayout> {
        Binding(get: { layoutStore.layout }, set: { layoutStore.set($0) })
    }

    private var sidebarVibrancyEnabled: Binding<Bool> {
        Binding(
            get: { AppBackgroundStyle.resolve(appBackgroundStyleRaw) == .vibrant },
            set: { appBackgroundStyleRaw = ($0 ? AppBackgroundStyle.vibrant : .solid).rawValue }
        )
    }

    private var isProjectFocused: Bool {
        layoutStore.layout == .projectFocused
    }

    private var sidebarProviders: [ExtensionStore.ExtensionStatus] {
        SidebarSelection.availableProviders(store: extensionStore)
    }

    private var localizationProviders: [ExtensionStore.LocalizationBinding] {
        LocalizationSelection.availableProviders(store: extensionStore)
    }

    private var localizationOptions: [LocalizationSelection.Option] {
        LocalizationSelection.options(
            from: localizationProviders,
            selectedValue: selectedLocalization
        )
    }

    private var localizationSelection: Binding<String> {
        Binding(
            get: { selectedLocalization },
            set: {
                selectedLocalization = $0
                localizationService.select($0, store: extensionStore)
            }
        )
    }

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Language",
                footer: localizationFooter
            ) {
                SettingsRow("App Language") {
                    Picker("", selection: localizationSelection) {
                        ForEach(localizationOptions) { option in
                            if option.id == LocalizationSelection.builtinValue {
                                Text(L10n.resource("English"))
                                    .tag(option.id)
                            } else {
                                Text(verbatim: option.title)
                                    .tag(option.id)
                                    .disabled(!option.isAvailable)
                            }
                        }
                    }
                    .labelsHidden()
                    .settingsControl()
                }
                SettingsRow("More Languages") {
                    Button {
                        ExtensionsPresentationRequest(
                            browseCategory: ExtensionMarketplaceCategory.localization
                        ).post()
                    } label: {
                        Text(L10n.resource("Browse Language Extensions…"))
                    }
                    .buttonStyle(.plain)
                    .font(.system(size: SettingsMetrics.labelFontSize, weight: .medium))
                    .foregroundStyle(SettingsStyle.accent)
                    .help(L10n.string("Find and install language packs from the Extension Store."))
                }
            }

            SettingsSection("Layout") {
                SettingsRow("App Layout") {
                    AppLayoutPicker(selection: layoutSelection)
                }
            }

            SettingsSection(
                "Appearance",
                footer: """
                Transparency shows the desktop through terminal panes, the top bar, and the status bar. \
                Vibrancy controls the native macOS material intensity and is required for the effect.
                """
            ) {
                BackgroundAppearanceSliders(
                    transparency: $appTransparency,
                    blurIntensity: $appBlurIntensity,
                    transparencyLabel: "App transparency",
                    vibrancyLabel: "App vibrancy",
                    defaultTransparency: AppTransparencyPreferences.defaultTransparency,
                    defaultBlurIntensity: AppTransparencyPreferences.defaultBlurIntensity
                )
            }

            sidebarSection

            SettingsSection("Theme") {
                SettingsRow("Light Theme") {
                    themeButton(
                        title: currentLightTheme ?? L10n.string("Default"),
                        isPresented: $showLightThemePicker,
                        mode: .light
                    )
                }
                SettingsRow("Dark Theme") {
                    themeButton(
                        title: currentDarkTheme ?? L10n.string("Default"),
                        isPresented: $showDarkThemePicker,
                        mode: .dark
                    )
                }
            }

            SettingsSection("Interface", showsDivider: false) {
                SettingsRow("Size") {
                    Picker("", selection: $uiScale.preset) {
                        ForEach(UIScale.Preset.allCases) { preset in
                            Text(L10n.resource(key: preset.title)).tag(preset)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.segmented)
                    .settingsControl(.intrinsic)
                }

                TabHeaderWidthSettingRow()

                SettingsToggleRow(label: L10n.resource("Show Status Bar"), isOn: $showStatusBar)

                SettingsToggleRow(label: L10n.resource("Show Resource Usage in Status Bar"), isOn: $showResourceUsage)
            }
        }
        .task {
            refreshThemeNames()
        }
        .onReceive(NotificationCenter.default.publisher(for: .themeDidChange)) { _ in
            refreshThemeNames()
        }
    }

    private var localizationFooter: LocalizedStringResource {
        if localizationOptions.contains(where: { $0.id == selectedLocalization && !$0.isAvailable }) {
            return "The selected language extension is unavailable, so Muxy is temporarily using English."
        }
        return "English is built in. Additional languages can be provided by enabled extensions."
    }

    @ViewBuilder
    private var sidebarSection: some View {
        if !sidebarProviders.isEmpty {
            SettingsSection("Active Sidebar") {
                SettingsRow("Sidebar") {
                    Picker("", selection: $activeSidebar) {
                        Text(L10n.resource("Built-in")).tag(SidebarSelection.builtinValue)
                        ForEach(sidebarProviders) { status in
                            Text(label(for: status)).tag(status.id)
                        }
                    }
                    .labelsHidden()
                    .settingsControl()
                }
            }
        }

        SettingsSection("Sidebar") {
            SettingsToggleRow(label: L10n.resource("Vibrancy"), isOn: sidebarVibrancyEnabled)

            SettingsToggleRow(label: L10n.resource("Show Home"), isOn: $showHomeProject)

            SettingsToggleRow(label: L10n.resource("Show Tips"), isOn: $showTips)

            SettingsToggleRow(
                label: L10n.resource("Auto-expand worktrees on project switch"),
                isOn: $autoExpandWorktrees
            )

            if isProjectFocused {
                SettingsToggleRow(
                    label: L10n.resource("Always Show Project Search"),
                    isOn: $showProjectSearch
                )

                SettingsRow("Collapsed Style") {
                    Picker("", selection: $sidebarCollapsedStyle) {
                        ForEach(SidebarCollapsedStyle.allCases) { style in
                            Text(L10n.resource(key: style.title)).tag(style.rawValue)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.segmented)
                    .settingsControl(.intrinsic)
                }

                SettingsRow("Expanded Style") {
                    Picker("", selection: $sidebarExpandedStyle) {
                        ForEach(SidebarExpandedStyle.allCases) { style in
                            Text(L10n.resource(key: style.title)).tag(style.rawValue)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.segmented)
                    .settingsControl(.intrinsic)
                }
            }

            if layoutStore.layout.supportsGroupedWorktrees {
                SettingsToggleRow(
                    label: L10n.resource("Nest worktrees inside projects"),
                    isOn: $groupWorktrees
                )
            }
        }

        SettingsSection("Worktrees") {
            SettingsToggleRow(
                label: L10n.resource("Show unread notification indicator on worktrees"),
                isOn: $showWorktreeUnreadIndicator
            )

            SettingsToggleRow(
                label: L10n.resource("Order worktrees by most-recently-used"),
                isOn: $orderWorktreesByMRU
            )
        }
    }

    private func label(for status: ExtensionStore.ExtensionStatus) -> String {
        status.muxyExtension.manifest.sidebar?.title ?? status.muxyExtension.displayName
    }

    private func themeButton(
        title: String,
        isPresented: Binding<Bool>,
        mode: ThemePickerMode
    ) -> some View {
        Button {
            isPresented.wrappedValue.toggle()
        } label: {
            HStack(spacing: 6) {
                Text(title)
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .lineLimit(1)
                Image(systemName: "chevron.up.chevron.down")
                    .font(.system(size: 10))
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 5)
            .foregroundStyle(SettingsStyle.foreground)
            .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: 6))
        }
        .buttonStyle(.plain)
        .popover(isPresented: isPresented) {
            ThemePicker(mode: mode)
                .environment(themeService)
        }
    }

    private func refreshThemeNames() {
        currentLightTheme = themeService.currentLightThemeName()
        currentDarkTheme = themeService.currentDarkThemeName()
    }
}

struct AppLayoutPicker: View {
    @Binding var selection: AppLayout

    var body: some View {
        Picker("", selection: $selection) {
            ForEach(AppLayout.allCases) { layout in
                Text(L10n.resource(key: layout.title)).tag(layout)
            }
        }
        .labelsHidden()
        .pickerStyle(.segmented)
        .settingsControl(.intrinsic)
    }
}

private struct TabHeaderWidthSettingRow: View {
    @AppStorage(TabWidthPreferences.maxWidthKey) private var maxTabWidth = TabWidthPreferences.defaultMaxWidth

    private var sliderValue: Binding<Double> {
        Binding(
            get: { TabWidthPreferences.sliderValue(from: maxTabWidth) },
            set: { maxTabWidth = TabWidthPreferences.storedValue(forSlider: $0.rounded()) }
        )
    }

    private var valueLabel: String {
        TabWidthPreferences.effectiveMaxWidth(from: maxTabWidth)
            .map { "\(Int($0))px" } ?? "Full-width"
    }

    var body: some View {
        SettingsRow("Tab header width") {
            HStack(spacing: UIMetrics.spacing3) {
                Slider(
                    value: sliderValue,
                    in: TabWidthPreferences.minMaxWidth ... TabWidthPreferences.maxMaxWidth
                )
                Text(valueLabel)
                    .font(.system(size: SettingsMetrics.labelFontSize).monospacedDigit())
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .frame(width: 64, alignment: .trailing)
            }
            .settingsControl()
        }
    }
}

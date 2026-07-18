import SwiftUI

struct InterfaceSettingsView: View {
    @State private var uiScale = UIScale.shared
    @State private var themeService = ThemeService.shared
    @State private var extensionStore = ExtensionStore.shared
    @State private var showLightThemePicker = false
    @State private var showDarkThemePicker = false
    @State private var currentLightTheme: String?
    @State private var currentDarkTheme: String?
    @AppStorage(AppBackgroundStyle.storageKey)
    private var appBackgroundStyleRaw = AppBackgroundStyle.defaultValue.rawValue
    @AppStorage("muxy.showStatusBar") private var showStatusBar = true
    @AppStorage(ResourceUsagePreferences.visibleKey) private var showResourceUsage = ResourceUsagePreferences.defaultVisible
    @State private var layoutStore = AppLayoutStore.shared
    @AppStorage(GeneralSettingsKeys.autoExpandWorktreesOnProjectSwitch) private var autoExpandWorktrees = false
    @AppStorage(SidebarCollapsedStyle.storageKey) private var sidebarCollapsedStyle = SidebarCollapsedStyle.defaultValue.rawValue
    @AppStorage(SidebarExpandedStyle.storageKey) private var sidebarExpandedStyle = SidebarExpandedStyle.defaultValue.rawValue
    @AppStorage(HomeProjectPreferences.visibleKey) private var showHomeProject = HomeProjectPreferences.defaultVisible
    @AppStorage(SidebarSelection.storageKey) private var activeSidebar = SidebarSelection.builtinValue
    @AppStorage(WorktreeListPreferences.showUnreadIndicatorKey)
    private var showWorktreeUnreadIndicator = WorktreeListPreferences.defaultShowUnreadIndicator
    @AppStorage(WorktreeListPreferences.orderByMRUKey)
    private var orderWorktreesByMRU = WorktreeListPreferences.defaultOrderByMRU

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

    var body: some View {
        SettingsContainer {
            SettingsSection("Layout") {
                SettingsRow("App Layout") {
                    Picker("", selection: layoutSelection) {
                        ForEach(AppLayout.allCases) { layout in
                            Text(layout.title).tag(layout)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.segmented)
                    .frame(width: SettingsMetrics.controlWidth)
                }
            }

            sidebarSection

            SettingsSection("Theme") {
                SettingsRow("Light Theme") {
                    themeButton(
                        title: currentLightTheme ?? "Default",
                        isPresented: $showLightThemePicker,
                        mode: .light
                    )
                }
                SettingsRow("Dark Theme") {
                    themeButton(
                        title: currentDarkTheme ?? "Default",
                        isPresented: $showDarkThemePicker,
                        mode: .dark
                    )
                }
            }

            SettingsSection("Interface", showsDivider: false) {
                SettingsRow("Size") {
                    Picker("", selection: $uiScale.preset) {
                        ForEach(UIScale.Preset.allCases) { preset in
                            Text(preset.title).tag(preset)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.segmented)
                }

                TabHeaderWidthSettingRow()

                SettingsToggleRow(label: "Show Status Bar", isOn: $showStatusBar)

                SettingsToggleRow(label: "Show Resource Usage in Status Bar", isOn: $showResourceUsage)
            }
        }
        .task {
            refreshThemeNames()
        }
        .onReceive(NotificationCenter.default.publisher(for: .themeDidChange)) { _ in
            refreshThemeNames()
        }
    }

    @ViewBuilder
    private var sidebarSection: some View {
        if !sidebarProviders.isEmpty {
            SettingsSection("Active Sidebar") {
                SettingsRow("Sidebar") {
                    HStack {
                        Spacer()
                        Picker("", selection: $activeSidebar) {
                            Text("Built-in").tag(SidebarSelection.builtinValue)
                            ForEach(sidebarProviders) { status in
                                Text(label(for: status)).tag(status.id)
                            }
                        }
                        .labelsHidden()
                        .fixedSize()
                    }
                    .frame(width: SettingsMetrics.controlWidth)
                }
            }
        }

        SettingsSection("Sidebar") {
            SettingsToggleRow(label: "Vibrancy", isOn: sidebarVibrancyEnabled)

            SettingsToggleRow(label: "Show Home", isOn: $showHomeProject)

            SettingsToggleRow(
                label: "Auto-expand worktrees on project switch",
                isOn: $autoExpandWorktrees
            )

            if isProjectFocused {
                SettingsRow("Collapsed Style") {
                    HStack {
                        Spacer()
                        Picker("", selection: $sidebarCollapsedStyle) {
                            ForEach(SidebarCollapsedStyle.allCases) { style in
                                Text(style.title).tag(style.rawValue)
                            }
                        }
                        .labelsHidden()
                        .pickerStyle(.segmented)
                        .fixedSize()
                    }
                    .frame(width: SettingsMetrics.controlWidth)
                }

                SettingsRow("Expanded Style") {
                    HStack {
                        Spacer()
                        Picker("", selection: $sidebarExpandedStyle) {
                            ForEach(SidebarExpandedStyle.allCases) { style in
                                Text(style.title).tag(style.rawValue)
                            }
                        }
                        .labelsHidden()
                        .pickerStyle(.segmented)
                        .fixedSize()
                    }
                    .frame(width: SettingsMetrics.controlWidth)
                }
            }
        }

        SettingsSection("Worktrees") {
            SettingsToggleRow(
                label: "Show unread notification indicator on worktrees",
                isOn: $showWorktreeUnreadIndicator
            )

            SettingsToggleRow(
                label: "Order worktrees by most-recently-used",
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
            .frame(width: SettingsMetrics.controlWidth)
        }
    }
}

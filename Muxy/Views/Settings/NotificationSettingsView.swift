import SwiftUI

struct NotificationSettingsView: View {
    @AppStorage(NotificationSettings.Key.sound) private var sound = NotificationSettings.Default.sound.rawValue
    @AppStorage(NotificationSettings.Key.toastEnabled) private var toastEnabled = NotificationSettings.Default.toastEnabled
    @AppStorage(NotificationSettings.Key.desktopEnabled) private var desktopEnabled = NotificationSettings.Default.desktopEnabled
    @AppStorage(NotificationSettings.Key.toastPosition) private var toastPosition = NotificationSettings.Default.toastPosition.rawValue

    var body: some View {
        SettingsContainer {
            SettingsSection("Delivery") {
                SettingsToggleRow(label: L10n.resource("Toast"), isOn: $toastEnabled)
                SettingsToggleRow(label: L10n.resource("Desktop notifications"), isOn: $desktopEnabled)
                    .onChange(of: desktopEnabled) { _, newValue in
                        requestDesktopNotificationAuthorizationIfNeeded(newValue)
                    }
            }

            SettingsSection("Sound") {
                SettingsPickerRow<NotificationSound>(
                    label: L10n.resource("Sound"),
                    selection: $sound,
                    width: 160
                )
                .onChange(of: sound) { _, newValue in
                    previewSound(newValue)
                }
            }

            SettingsSection("Toast") {
                SettingsPickerRow<ToastPosition>(
                    label: L10n.resource("Position"),
                    selection: $toastPosition,
                    width: 160
                )
            }

            SettingsSection("AI Providers", showsDivider: false) {
                ForEach(AIProviderRegistry.shared.providers, id: \.id) { provider in
                    ProviderToggleRow(provider: provider)
                }
            }
        }
    }

    private func previewSound(_ value: String) {
        guard let sound = NotificationSound.playableSound(for: value) else { return }
        NotificationSoundPlayer.shared.play(sound)
    }

    private func requestDesktopNotificationAuthorizationIfNeeded(_ enabled: Bool) {
        guard enabled else { return }
        DesktopNotificationService.shared.requestAuthorizationIfNeeded { authorized in
            if !authorized {
                desktopEnabled = false
            }
        }
    }
}

private struct ProviderToggleRow: View {
    let provider: AIProviderIntegration
    @State private var enabled: Bool
    @State private var refreshed = false
    @State private var testResult: HookTestResult?
    @State private var testing = false
    private let healthStore = HookHealthStore.shared

    init(provider: AIProviderIntegration) {
        self.provider = provider
        _enabled = State(initialValue: provider.isEnabled)
    }

    private var health: HookHealth {
        healthStore.health(for: provider.id)
    }

    private var isCLIMissing: Bool {
        health.installState == .cliMissing
    }

    var body: some View {
        TimelineView(.periodic(from: .now, by: 60)) { context in
            rowContent(now: context.date)
        }
    }

    private func rowContent(now: Date) -> some View {
        HStack(alignment: .top, spacing: 8) {
            statusDot(now: now)
                .padding(.top, 4)
            VStack(alignment: .leading, spacing: 2) {
                Text(provider.displayName)
                    .font(.system(size: SettingsMetrics.labelFontSize))
                if enabled {
                    Text(secondaryLine(now: now))
                        .font(.system(size: SettingsMetrics.footnoteFontSize))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                    if let discoveryLine = HookHealthPresenter.discoveryLine(for: health) {
                        Text(discoveryLine)
                            .font(.system(size: SettingsMetrics.footnoteFontSize))
                            .foregroundStyle(SettingsStyle.dimForeground)
                            .help(health.discovery?.executablePath ?? "")
                    }
                }
            }
            Spacer()
            if enabled {
                testButton
                refreshButton
            }
            Toggle("", isOn: $enabled)
                .labelsHidden()
                .toggleStyle(.switch)
                .controlSize(.small)
                .onChange(of: enabled) { _, newValue in
                    provider.isEnabled = newValue
                    testResult = nil
                    Task { @MainActor in
                        await AIProviderRegistry.shared.installAll()
                    }
                }
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }

    private func statusDot(now: Date) -> some View {
        Circle()
            .fill(dotColor(now: now))
            .frame(width: 7, height: 7)
    }

    private func dotColor(now: Date) -> Color {
        switch HookHealthPresenter.dot(for: health, now: now) {
        case .healthy: MuxyTheme.diffAddFg
        case .warning: SettingsStyle.warning
        case .error: MuxyTheme.diffRemoveFg
        case .idle: SettingsStyle.dimForeground
        }
    }

    private func secondaryLine(now: Date) -> String {
        if let testResult {
            switch testResult {
            case .passed: return L10n.string("Test passed")
            case let .failed(reason): return L10n.string("Test failed — \(reason)")
            }
        }
        return HookHealthPresenter.statusLine(for: health, now: now)
    }

    private var testButton: some View {
        Button {
            runTest()
        } label: {
            if testing {
                Text(L10n.resource("Testing…"))
            } else {
                Text(L10n.resource("Test"))
            }
        }
        .buttonStyle(.plain)
        .font(.system(size: SettingsMetrics.footnoteFontSize))
        .foregroundStyle(SettingsStyle.accent)
        .disabled(testing || isCLIMissing)
    }

    private var refreshButton: some View {
        Button {
            Task { @MainActor in
                await AIProviderRegistry.shared.forceInstall(provider)
                withAnimation { refreshed = true }
                try? await Task.sleep(for: .seconds(2))
                withAnimation { refreshed = false }
            }
        } label: {
            if refreshed {
                Label(L10n.string("Done"), systemImage: "checkmark")
            } else {
                Text(L10n.resource("Refresh"))
            }
        }
        .buttonStyle(.plain)
        .font(.system(size: SettingsMetrics.footnoteFontSize))
        .foregroundStyle(refreshed ? MuxyTheme.diffAddFg : SettingsStyle.accent)
        .disabled(refreshed)
    }

    private func runTest() {
        testing = true
        testResult = nil
        let socketType = provider.socketTypeKey
        let title = provider.displayName
        Task {
            let result = await Task.detached(priority: .userInitiated) {
                HookTestRunner().run(providerSocketType: socketType, providerTitle: title)
            }.value
            await MainActor.run {
                testResult = result
                testing = false
            }
        }
    }
}

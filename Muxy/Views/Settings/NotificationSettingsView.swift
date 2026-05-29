import SwiftUI

struct NotificationSettingsView: View {
    @AppStorage("muxy.notifications.sound") private var sound = NotificationSound.funk.rawValue
    @AppStorage("muxy.notifications.toastEnabled") private var toastEnabled = true
    @AppStorage("muxy.notifications.toastPosition") private var toastPosition = ToastPosition.topCenter.rawValue

    var body: some View {
        SettingsContainer {
            SettingsSection("Delivery") {
                SettingsToggleRow(label: "Toast", isOn: $toastEnabled)
            }

            SettingsSection("Sound") {
                SettingsPickerRow<NotificationSound>(
                    label: "Sound",
                    selection: $sound,
                    width: 160
                )
                .onChange(of: sound) { _, newValue in
                    previewSound(newValue)
                }
            }

            SettingsSection("Toast") {
                SettingsPickerRow<ToastPosition>(
                    label: "Position",
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
}

private struct ProviderToggleRow: View {
    let provider: AIProviderIntegration
    @State private var enabled: Bool
    @State private var refreshed = false

    init(provider: AIProviderIntegration) {
        self.provider = provider
        _enabled = State(initialValue: provider.isEnabled)
    }

    var body: some View {
        HStack {
            Text(provider.displayName)
                .font(.system(size: SettingsMetrics.labelFontSize))
            Spacer()
            if enabled {
                Button {
                    AIProviderRegistry.shared.forceInstall(provider)
                    withAnimation { refreshed = true }
                    Task {
                        try? await Task.sleep(for: .seconds(2))
                        withAnimation { refreshed = false }
                    }
                } label: {
                    if refreshed {
                        Label("Done", systemImage: "checkmark")
                    } else {
                        Text("Refresh")
                    }
                }
                .buttonStyle(.plain)
                .font(.system(size: SettingsMetrics.footnoteFontSize))
                .foregroundStyle(refreshed ? MuxyTheme.diffAddFg : SettingsStyle.accent)
                .disabled(refreshed)
            }
            Toggle("", isOn: $enabled)
                .labelsHidden()
                .toggleStyle(.switch)
                .controlSize(.small)
                .onChange(of: enabled) { _, newValue in
                    provider.isEnabled = newValue
                    AIProviderRegistry.shared.installAll()
                }
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }
}

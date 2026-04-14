import AppKit
import SwiftUI

struct NotificationSettingsView: View {
    @AppStorage("muxy.notifications.sound") private var sound = NotificationSound.funk.rawValue
    @AppStorage("muxy.notifications.toastEnabled") private var toastEnabled = true
    @AppStorage("muxy.notifications.toastPosition") private var toastPosition = ToastPosition.topCenter.rawValue

    var body: some View {
        VStack(spacing: 0) {
            section("Delivery") {
                toggleRow("Toast", isOn: $toastEnabled)
            }

            Divider().padding(.horizontal, 12)

            section("Sound") {
                pickerRow("Sound", selection: $sound, options: NotificationSound.allCases) { $0.rawValue }
                    .onChange(of: sound) { _, newValue in
                        previewSound(newValue)
                    }
            }

            Divider().padding(.horizontal, 12)

            section("Toast") {
                pickerRow("Position", selection: $toastPosition, options: ToastPosition.allCases) { $0.rawValue }
            }

            Divider().padding(.horizontal, 12)

            section("AI Providers") {
                ForEach(AIProviderRegistry.shared.providers, id: \.id) { provider in
                    ProviderToggleRow(provider: provider)
                }
            }

            Spacer()
        }
    }

    private func section(_ title: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(title)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.secondary)
                .padding(.horizontal, 12)
                .padding(.top, 10)
                .padding(.bottom, 4)
            content()
        }
    }

    private func toggleRow(_ label: String, isOn: Binding<Bool>) -> some View {
        HStack {
            Text(label)
                .font(.system(size: 12))
            Spacer()
            Toggle("", isOn: isOn)
                .toggleStyle(.switch)
                .controlSize(.small)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
    }

    private func pickerRow<T: Identifiable & RawRepresentable<String>>(
        _ label: String,
        selection: Binding<String>,
        options: [T],
        displayValue: @escaping (T) -> String
    ) -> some View {
        HStack {
            Text(label)
                .font(.system(size: 12))
            Spacer()
            Picker("", selection: selection) {
                ForEach(options) { option in
                    Text(displayValue(option)).tag(option.rawValue)
                }
            }
            .frame(width: 160)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
    }

    private func previewSound(_ value: String) {
        guard let sound = NotificationSound(rawValue: value), sound != .none else { return }
        NSSound(named: .init(sound.rawValue))?.play()
    }
}

private struct ProviderToggleRow: View {
    let provider: AIProviderIntegration
    @State private var enabled: Bool

    init(provider: AIProviderIntegration) {
        self.provider = provider
        _enabled = State(initialValue: provider.isEnabled)
    }

    var body: some View {
        HStack {
            Image(systemName: provider.iconName)
                .font(.system(size: 10))
                .foregroundStyle(.secondary)
                .frame(width: 16)
            Text(provider.displayName)
                .font(.system(size: 12))
            Spacer()
            Toggle("", isOn: $enabled)
                .toggleStyle(.switch)
                .controlSize(.small)
                .onChange(of: enabled) { _, newValue in
                    provider.isEnabled = newValue
                    AIProviderRegistry.shared.installAll()
                }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
    }
}

import SwiftUI

struct ExtensionCustomSettingsView: View {
    let extensionID: String
    @Environment(ExtensionStore.self) private var store
    @Environment(ExtensionSettingsStore.self) private var settingsStore

    private var muxyExtension: MuxyExtension? {
        store.statuses.first(where: { $0.id == extensionID })?.muxyExtension
    }

    var body: some View {
        SettingsContainer {
            if let muxyExtension {
                if muxyExtension.manifest.settings.isEmpty {
                    SettingsSection("Settings") {
                        Text("This extension does not declare any settings.")
                            .font(.system(size: SettingsMetrics.footnoteFontSize))
                            .foregroundStyle(SettingsStyle.mutedForeground)
                            .padding(.horizontal, SettingsMetrics.horizontalPadding)
                            .padding(.vertical, SettingsMetrics.rowVerticalPadding)
                    }
                } else {
                    SettingsSection(muxyExtension.displayName) {
                        ForEach(muxyExtension.manifest.settings) { entry in
                            ExtensionSettingRow(
                                extensionID: extensionID,
                                entry: entry,
                                settingsStore: settingsStore
                            )
                        }
                    }
                }
            } else {
                Text("Extension is not loaded.")
                    .font(.system(size: SettingsMetrics.footnoteFontSize))
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .padding(.horizontal, SettingsMetrics.horizontalPadding)
                    .padding(.vertical, SettingsMetrics.rowVerticalPadding)
            }
        }
    }
}

private struct ExtensionSettingRow: View {
    let extensionID: String
    let entry: ExtensionSettingEntry
    let settingsStore: ExtensionSettingsStore

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            SettingsRow(entry.title) {
                control
            }
            if let description = entry.description, !description.isEmpty {
                Text(description)
                    .font(.system(size: SettingsMetrics.footnoteFontSize))
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .padding(.horizontal, SettingsMetrics.horizontalPadding)
                    .padding(.bottom, SettingsMetrics.rowVerticalPadding)
            }
        }
    }

    @ViewBuilder
    private var control: some View {
        switch entry.type {
        case .bool:
            Toggle("", isOn: boolBinding)
                .labelsHidden()
                .toggleStyle(.switch)
                .controlSize(.small)
        case .string:
            TextField("", text: stringBinding)
                .settingsTextInput(width: SettingsMetrics.controlWidth)
        case .number:
            TextField("", text: numberBinding)
                .settingsTextInput(width: SettingsMetrics.controlWidth)
        }
    }

    private var boolBinding: Binding<Bool> {
        Binding(
            get: {
                guard let value = settingsStore.effectiveValue(extensionID: extensionID, entry: entry) else { return false }
                if case let .bool(value) = value {
                    return value
                }
                return false
            },
            set: { newValue in
                settingsStore.setValue(.bool(newValue), extensionID: extensionID, key: entry.key)
            }
        )
    }

    private var stringBinding: Binding<String> {
        Binding(
            get: {
                guard let value = settingsStore.effectiveValue(extensionID: extensionID, entry: entry) else { return "" }
                if case let .string(value) = value {
                    return value
                }
                return ""
            },
            set: { newValue in
                settingsStore.setValue(.string(newValue), extensionID: extensionID, key: entry.key)
            }
        )
    }

    private var numberBinding: Binding<String> {
        Binding(
            get: {
                guard let value = settingsStore.effectiveValue(extensionID: extensionID, entry: entry) else { return "" }
                if case let .number(value) = value {
                    return value.truncatingRemainder(dividingBy: 1) == 0
                        ? String(Int(value))
                        : String(value)
                }
                return ""
            },
            set: { newValue in
                let trimmed = newValue.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !trimmed.isEmpty, let parsed = Double(trimmed) else {
                    settingsStore.setValue(nil, extensionID: extensionID, key: entry.key)
                    return
                }
                settingsStore.setValue(.number(parsed), extensionID: extensionID, key: entry.key)
            }
        )
    }
}

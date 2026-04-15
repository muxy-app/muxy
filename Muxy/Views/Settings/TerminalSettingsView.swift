import SwiftUI

struct TerminalSettingsView: View {
    @AppStorage(TerminalSettings.quickSelectLayoutKey) private var quickSelectLayout = QuickSelectKeyboardLayout.qwerty.rawValue
    @AppStorage(TerminalSettings.quickSelectCustomLabelsKey) private var customLabels = QuickSelectKeyboardLayout.colemakDH.defaultLabels

    var body: some View {
        VStack(spacing: 0) {
            section("Quick Select") {
                pickerRow("Label Layout", selection: $quickSelectLayout, options: QuickSelectKeyboardLayout.allCases) {
                    $0.displayName
                }

                if quickSelectLayout == QuickSelectKeyboardLayout.custom.rawValue {
                    customLabelsRow
                } else {
                    previewRow
                }
            }

            Spacer()
        }
    }

    private var customLabelsRow: some View {
        HStack {
            Text("Custom Labels")
                .font(.system(size: 12))
            Spacer()
            TextField("", text: customLabelsBinding)
                .textFieldStyle(.roundedBorder)
                .font(.system(size: 12, design: .monospaced))
                .frame(width: 250)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
    }

    private var previewRow: some View {
        HStack {
            Text("Label Order")
                .font(.system(size: 12))
            Spacer()
            Text(activeLayout.defaultLabels)
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(.secondary)
                .lineLimit(1)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
    }

    private var customLabelsBinding: Binding<String> {
        Binding(
            get: { customLabels },
            set: { customLabels = TerminalSettings.sanitizeQuickSelectLabelString($0) }
        )
    }

    private var activeLayout: QuickSelectKeyboardLayout {
        QuickSelectKeyboardLayout(rawValue: quickSelectLayout) ?? .qwerty
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
}

import SwiftUI

enum SettingsMetrics {
    static let horizontalPadding: CGFloat = 12
    static let rowVerticalPadding: CGFloat = 6
    static let sectionHeaderTopPadding: CGFloat = 10
    static let sectionHeaderBottomPadding: CGFloat = 4
    static let sectionFooterTopPadding: CGFloat = 6
    static let labelFontSize: CGFloat = 12
    static let sectionTitleFontSize: CGFloat = 11
    static let footnoteFontSize: CGFloat = 11
    static let controlWidth: CGFloat = 210
}

struct SettingsContainer<Content: View>: View {
    @ViewBuilder var content: Content

    var body: some View {
        VStack(spacing: 0) {
            content
            Spacer(minLength: 0)
        }
    }
}

struct SettingsSection<Content: View>: View {
    let title: String
    let footer: String?
    let showsDivider: Bool
    @ViewBuilder var content: Content

    init(
        _ title: String,
        footer: String? = nil,
        showsDivider: Bool = true,
        @ViewBuilder content: () -> Content
    ) {
        self.title = title
        self.footer = footer
        self.showsDivider = showsDivider
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(title)
                .font(.system(size: SettingsMetrics.sectionTitleFontSize, weight: .semibold))
                .foregroundStyle(.secondary)
                .padding(.horizontal, SettingsMetrics.horizontalPadding)
                .padding(.top, SettingsMetrics.sectionHeaderTopPadding)
                .padding(.bottom, SettingsMetrics.sectionHeaderBottomPadding)

            content

            if let footer {
                Text(footer)
                    .font(.system(size: SettingsMetrics.footnoteFontSize))
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.horizontal, SettingsMetrics.horizontalPadding)
                    .padding(.top, SettingsMetrics.sectionFooterTopPadding)
                    .padding(.bottom, SettingsMetrics.sectionHeaderBottomPadding)
            }

            if showsDivider {
                Divider().padding(.horizontal, SettingsMetrics.horizontalPadding)
            }
        }
    }
}

struct SettingsRow<Content: View>: View {
    let label: String
    @ViewBuilder var content: Content

    init(_ label: String, @ViewBuilder content: () -> Content) {
        self.label = label
        self.content = content()
    }

    var body: some View {
        HStack {
            Text(label)
                .font(.system(size: SettingsMetrics.labelFontSize))
            Spacer()
            content
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }
}

struct SettingsToggleRow: View {
    let label: String
    @Binding var isOn: Bool

    var body: some View {
        SettingsRow(label) {
            Toggle("", isOn: $isOn)
                .labelsHidden()
                .toggleStyle(.switch)
                .controlSize(.small)
        }
    }
}

struct SettingsPickerRow<Value: Hashable, Options: RandomAccessCollection>: View where Options.Element: Identifiable {
    let label: String
    @Binding var selection: Value
    let options: Options
    let tag: (Options.Element) -> Value
    let display: (Options.Element) -> String
    var width: CGFloat = SettingsMetrics.controlWidth

    var body: some View {
        SettingsRow(label) {
            Picker("", selection: $selection) {
                ForEach(options) { option in
                    Text(display(option)).tag(tag(option))
                }
            }
            .labelsHidden()
            .frame(width: width, alignment: .trailing)
        }
    }
}

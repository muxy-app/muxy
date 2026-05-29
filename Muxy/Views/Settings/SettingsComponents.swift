import AppKit
import SwiftUI

extension EnvironmentValues {
    @Entry var settingsSearchQuery: String = ""

    @Entry var settingsCategory: SettingsCategory?
}

enum SettingsMetrics {
    static let horizontalPadding: CGFloat = 12
    static let verticalPadding: CGFloat = 12
    static let rowVerticalPadding: CGFloat = 6
    static let sectionHeaderTopPadding: CGFloat = 10
    static let sectionHeaderBottomPadding: CGFloat = 4
    static let sectionFooterTopPadding: CGFloat = 6
    static let sectionFooterBottomPadding: CGFloat = 10
    static let labelFontSize: CGFloat = 12
    static let footnoteFontSize: CGFloat = 11
    static let controlWidth: CGFloat = 210
}

enum SettingsStyle {
    @MainActor static var background: Color { MuxyTheme.bg }
    @MainActor static var foreground: Color { MuxyTheme.fg }
    @MainActor static var mutedForeground: Color { MuxyTheme.fgMuted }
    @MainActor static var dimForeground: Color { MuxyTheme.fgDim }
    @MainActor static var surface: Color { MuxyTheme.surface }
    @MainActor static var elevatedSurface: Color { MuxyTheme.surface.opacity(1.45) }
    @MainActor static var sidebarBackground: Color {
        Color(nsColor: MuxyTheme.nsBg.blended(withFraction: 0.08, of: .black) ?? MuxyTheme.nsBg)
    }

    @MainActor static var hover: Color { MuxyTheme.hover }
    @MainActor static var border: Color { MuxyTheme.border }
    @MainActor static var accent: Color { MuxyTheme.accent }
    @MainActor static var accentSoft: Color { MuxyTheme.accentSoft }
    @MainActor static var warning: Color { MuxyTheme.warning }
    @MainActor static var destructive: Color { MuxyTheme.diffRemoveFg }
    @MainActor static var destructiveSoft: Color { MuxyTheme.diffRemoveBg }
    @MainActor static var nsBackground: NSColor { MuxyTheme.nsBg }
    @MainActor static var nsForeground: NSColor { MuxyTheme.nsFg }
    @MainActor static var mutedNSForeground: NSColor { MuxyTheme.nsFgMuted }
}

struct SettingsDivider: View {
    var body: some View {
        Rectangle()
            .fill(SettingsStyle.border)
            .frame(height: 1)
    }
}

struct SettingsDevelopmentBadge: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 9, weight: .bold, design: .monospaced))
            .tracking(0.6)
            .foregroundStyle(SettingsStyle.warning)
            .padding(.horizontal, 5)
            .padding(.vertical, 1)
            .background(SettingsStyle.warning.opacity(0.15), in: RoundedRectangle(cornerRadius: 3))
            .overlay(
                RoundedRectangle(cornerRadius: 3)
                    .stroke(SettingsStyle.warning.opacity(0.4), lineWidth: 1)
            )
    }
}

struct SettingsContainer<Content: View>: View {
    @ViewBuilder var content: Content

    var body: some View {
        ScrollView {
            VStack(spacing: 0) {
                content
            }
            .frame(maxWidth: .infinity, alignment: .top)
        }
        .background(SettingsStyle.background)
    }
}

struct SettingsSection<Content: View>: View {
    @Environment(\.settingsSearchQuery) private var searchQuery
    @Environment(\.settingsCategory) private var category

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
        if SettingsCatalog.sectionMatches(query: searchQuery, category: category, section: title) {
            VStack(alignment: .leading, spacing: 0) {
                Text(title)
                    .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .semibold))
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .padding(.horizontal, SettingsMetrics.horizontalPadding)
                    .padding(.top, SettingsMetrics.sectionHeaderTopPadding)
                    .padding(.bottom, SettingsMetrics.sectionHeaderBottomPadding)

                content

                if let footer {
                    Text(footer)
                        .font(.system(size: SettingsMetrics.footnoteFontSize))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                        .fixedSize(horizontal: false, vertical: true)
                        .padding(.horizontal, SettingsMetrics.horizontalPadding)
                        .padding(.top, SettingsMetrics.sectionFooterTopPadding)
                        .padding(.bottom, SettingsMetrics.sectionFooterBottomPadding)
                }

                if showsDivider {
                    SettingsDivider().padding(.horizontal, SettingsMetrics.horizontalPadding)
                }
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
                .foregroundStyle(SettingsStyle.foreground)
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

struct SettingsPickerRow<Option: CaseIterable & Identifiable & RawRepresentable>: View
    where Option.RawValue == String, Option.AllCases: RandomAccessCollection
{
    let label: String
    @Binding var selection: String
    var width: CGFloat = SettingsMetrics.controlWidth

    var body: some View {
        SettingsRow(label) {
            Picker("", selection: $selection) {
                ForEach(Option.allCases) { option in
                    Text(option.rawValue).tag(option.rawValue)
                }
            }
            .labelsHidden()
            .frame(width: width, alignment: .trailing)
        }
    }
}

extension View {
    func settingsTextInput(width: CGFloat? = nil, maxWidth: CGFloat? = nil, minHeight: CGFloat? = nil) -> some View {
        textFieldStyle(.plain)
            .foregroundStyle(SettingsStyle.foreground)
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .frame(width: width)
            .frame(maxWidth: maxWidth, minHeight: minHeight)
            .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: 6))
            .overlay(
                RoundedRectangle(cornerRadius: 6)
                    .stroke(SettingsStyle.border, lineWidth: 1)
            )
    }

    func resetsSettingsFocusOnOutsideClick() -> some View {
        background(SettingsFocusResetView())
    }
}

private struct SettingsFocusResetView: NSViewRepresentable {
    func makeNSView(context: Context) -> SettingsFocusResetNSView {
        SettingsFocusResetNSView()
    }

    func updateNSView(_ nsView: SettingsFocusResetNSView, context: Context) {}
}

private final class SettingsFocusResetNSView: NSView {
    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(nil)
        super.mouseDown(with: event)
    }
}

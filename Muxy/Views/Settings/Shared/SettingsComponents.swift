import AppKit
import SwiftUI

extension EnvironmentValues {
    @Entry var settingsSearchQuery: String = ""

    @Entry var settingsCategory: SettingsCategory?
}

enum SettingsMetrics {
    static let minimumWindowWidth: CGFloat = 860
    static let sidebarWidth: CGFloat = 210
    static let dividerThickness: CGFloat = 1
    static let minimumContentWidth = minimumWindowWidth - sidebarWidth - dividerThickness
    static let horizontalPadding: CGFloat = 12
    static let verticalPadding: CGFloat = 12
    static let rowVerticalPadding: CGFloat = 6
    static let rowSpacing: CGFloat = 12
    static let sectionHeaderTopPadding: CGFloat = 10
    static let sectionHeaderBottomPadding: CGFloat = 4
    static let sectionFooterTopPadding: CGFloat = 6
    static let sectionFooterBottomPadding: CGFloat = 10
    static let labelFontSize: CGFloat = 12
    static let footnoteFontSize: CGFloat = 11
    static let controlWidth: CGFloat = 210
}

enum SettingsControlSizing {
    case fill
    case intrinsic
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
            .frame(height: SettingsMetrics.dividerThickness)
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

    let title: LocalizedStringResource?
    let footer: LocalizedStringResource?
    let verbatimTitle: String?
    let verbatimFooter: String?
    let showsDivider: Bool
    @ViewBuilder var content: Content

    init(
        _ title: LocalizedStringResource,
        footer: LocalizedStringResource? = nil,
        showsDivider: Bool = true,
        @ViewBuilder content: () -> Content
    ) {
        self.title = title
        self.footer = footer
        verbatimTitle = nil
        verbatimFooter = nil
        self.showsDivider = showsDivider
        self.content = content()
    }

    init(
        verbatim title: String,
        footer: String? = nil,
        showsDivider: Bool = true,
        @ViewBuilder content: () -> Content
    ) {
        self.title = nil
        self.footer = nil
        verbatimTitle = title
        verbatimFooter = footer
        self.showsDivider = showsDivider
        self.content = content()
    }

    var body: some View {
        if SettingsCatalog.sectionMatches(query: searchQuery, category: category, section: searchTitle) {
            VStack(alignment: .leading, spacing: 0) {
                sectionTitle
                    .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .semibold))
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .padding(.horizontal, SettingsMetrics.horizontalPadding)
                    .padding(.top, SettingsMetrics.sectionHeaderTopPadding)
                    .padding(.bottom, SettingsMetrics.sectionHeaderBottomPadding)

                content

                if footer != nil || verbatimFooter != nil {
                    sectionFooter
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

    private var searchTitle: String {
        title?.key ?? verbatimTitle ?? ""
    }

    @ViewBuilder
    private var sectionTitle: some View {
        if let title {
            Text(L10n.resource(title))
        } else if let verbatimTitle {
            Text(verbatim: verbatimTitle)
        }
    }

    @ViewBuilder
    private var sectionFooter: some View {
        if let footer {
            Text(L10n.resource(footer))
        } else if let verbatimFooter {
            Text(verbatim: verbatimFooter)
        }
    }
}

struct SettingsRow<Content: View>: View {
    let label: LocalizedStringResource?
    let verbatimLabel: String?
    @ViewBuilder var content: Content

    init(_ label: LocalizedStringResource, @ViewBuilder content: () -> Content) {
        self.label = label
        verbatimLabel = nil
        self.content = content()
    }

    init(verbatim label: String, @ViewBuilder content: () -> Content) {
        self.label = nil
        verbatimLabel = label
        self.content = content()
    }

    var body: some View {
        HStack(spacing: 0) {
            rowLabel
                .font(.system(size: SettingsMetrics.labelFontSize))
                .foregroundStyle(SettingsStyle.foreground)
                .lineLimit(2)
                .truncationMode(.tail)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: SettingsMetrics.rowSpacing)
            content
                .layoutPriority(1)
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }

    @ViewBuilder
    private var rowLabel: some View {
        if let label {
            Text(L10n.resource(label))
        } else if let verbatimLabel {
            Text(verbatim: verbatimLabel)
        }
    }
}

struct SettingsToggleRow: View {
    let label: LocalizedStringResource
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
    let label: LocalizedStringResource
    @Binding var selection: String
    var width: CGFloat = SettingsMetrics.controlWidth

    var body: some View {
        SettingsRow(label) {
            Picker("", selection: $selection) {
                ForEach(Option.allCases) { option in
                    Text(L10n.resource(key: option.rawValue)).tag(option.rawValue)
                }
            }
            .labelsHidden()
            .settingsControl(width: width)
        }
    }
}

extension View {
    @ViewBuilder
    func settingsControl(
        _ sizing: SettingsControlSizing = .fill,
        width: CGFloat = SettingsMetrics.controlWidth
    ) -> some View {
        switch sizing {
        case .fill:
            frame(maxWidth: width, alignment: .trailing)
        case .intrinsic:
            fixedSize(horizontal: true, vertical: false)
                .frame(minWidth: width, alignment: .trailing)
        }
    }

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

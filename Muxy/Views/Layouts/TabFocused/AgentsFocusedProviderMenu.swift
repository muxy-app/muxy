import SwiftUI

struct AgentsFocusedProviderMenu: View {
    let options: [AgentTabLaunchOption]
    let onSelect: (AgentTabLaunchOption) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(1)) {
            Text("New Agent Tab")
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .padding(.horizontal, UIMetrics.spacing3)
                .padding(.bottom, UIMetrics.spacing2)

            ForEach(options) { option in
                AgentsFocusedProviderRow(option: option) {
                    onSelect(option)
                }
            }
        }
        .padding(UIMetrics.spacing4)
        .fixedSize(horizontal: true, vertical: true)
        .background(MuxyTheme.bg)
    }
}

struct AgentsFocusedProviderLoadingMenu: View {
    var body: some View {
        HStack(spacing: UIMetrics.spacing3) {
            ProgressView()
                .controlSize(.small)
            Text("Checking remote providers…")
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
        .padding(UIMetrics.spacing4)
        .frame(width: UIMetrics.scaled(252), height: UIMetrics.scaled(68))
        .background(MuxyTheme.bg)
    }
}

private struct AgentsFocusedProviderRow: View {
    let option: AgentTabLaunchOption
    let action: () -> Void

    @State private var hovered = false

    private var foreground: Color {
        option.command == nil ? MuxyTheme.fgDim : MuxyTheme.fg
    }

    var body: some View {
        Button(action: action) {
            HStack(spacing: UIMetrics.spacing3) {
                ProviderIconView(
                    iconName: option.provider.iconName,
                    size: UIMetrics.iconMD,
                    monochromeTint: foreground,
                    forceMonochrome: true
                )
                Text(option.title)
                    .font(.system(size: UIMetrics.fontBody))
                    .lineLimit(1)
                Spacer(minLength: UIMetrics.spacing6)
            }
            .foregroundStyle(foreground)
            .padding(.horizontal, UIMetrics.spacing3)
            .frame(width: UIMetrics.scaled(220), height: UIMetrics.controlMedium, alignment: .leading)
            .background(
                hovered && option.command != nil ? MuxyTheme.hover : .clear,
                in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(option.command == nil)
        .onHover { hovered = $0 }
    }
}

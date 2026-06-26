import SwiftUI

struct OverviewSection<Content: View, Accessory: View>: View {
    let title: String
    let storageKey: String
    let defaultExpanded: Bool
    @ViewBuilder let accessory: () -> Accessory
    @ViewBuilder let content: () -> Content

    @State private var isExpanded: Bool

    init(
        title: String,
        storageKey: String,
        defaultExpanded: Bool = true,
        @ViewBuilder accessory: @escaping () -> Accessory = { EmptyView() },
        @ViewBuilder content: @escaping () -> Content
    ) {
        self.title = title
        self.storageKey = storageKey
        self.defaultExpanded = defaultExpanded
        self.accessory = accessory
        self.content = content
        _isExpanded = State(initialValue: UserDefaults.standard.object(forKey: storageKey) as? Bool ?? defaultExpanded)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            if isExpanded {
                content()
                    .padding(.horizontal, UIMetrics.spacing4)
                    .padding(.bottom, UIMetrics.spacing4)
            }
        }
    }

    private var header: some View {
        Button(action: toggle) {
            HStack(spacing: UIMetrics.spacing3) {
                Image(systemName: "chevron.right")
                    .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .rotationEffect(.degrees(isExpanded ? 90 : 0))
                    .frame(width: UIMetrics.scaled(12))

                Text(title.uppercased())
                    .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                    .tracking(0.5)
                    .foregroundStyle(MuxyTheme.fgMuted)

                Spacer(minLength: UIMetrics.spacing2)

                accessory()
            }
            .padding(.horizontal, UIMetrics.spacing4)
            .padding(.vertical, UIMetrics.spacing3)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(isExpanded ? "Collapse \(title)" : "Expand \(title)")
    }

    private func toggle() {
        withAnimation(.easeInOut(duration: 0.15)) {
            isExpanded.toggle()
        }
        UserDefaults.standard.set(isExpanded, forKey: storageKey)
    }
}

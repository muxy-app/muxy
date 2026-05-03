import SwiftUI

struct LayoutPickerMenu: View {
    @Environment(AppState.self) private var appState
    let projectID: UUID
    @State private var hovered = false

    var body: some View {
        let layouts = appState.availableLayouts(for: projectID)
        if !layouts.isEmpty {
            Menu {
                ForEach(layouts) { layout in
                    Button(layout.name) {
                        appState.requestApplyLayout(projectID: projectID, layoutName: layout.name)
                    }
                }
            } label: {
                Image(systemName: "rectangle.split.2x2")
                    .font(.system(size: 13, weight: .semibold))
                    .frame(width: 24, height: 24)
                    .contentShape(Rectangle())
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
            .tint(hovered ? MuxyTheme.fg : MuxyTheme.fgMuted)
            .foregroundStyle(hovered ? MuxyTheme.fg : MuxyTheme.fgMuted)
            .onHover { hovered = $0 }
            .help("Apply Layout")
            .accessibilityLabel("Apply Layout")
        }
    }
}

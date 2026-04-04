import SwiftUI

struct PRBadge: View {
    let info: GitRepositoryService.PRInfo
    @State private var hovered = false

    var body: some View {
        Button {
            guard let url = URL(string: info.url) else { return }
            NSWorkspace.shared.open(url)
        } label: {
            HStack(spacing: 4) {
                Image(systemName: "arrow.triangle.pull")
                    .font(.system(size: 9, weight: .bold))
                Text("#\(info.number)")
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
            }
            .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(
                RoundedRectangle(cornerRadius: 5)
                    .fill(MuxyTheme.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 5)
                    .stroke(MuxyTheme.border, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }
}

import SwiftUI

struct EmptyProjectPlaceholder: View {
    let project: Project
    let onCreateTab: () -> Void

    var body: some View {
        VStack(spacing: UIMetrics.spacing7) {
            Spacer()
            Image(systemName: "macwindow.badge.plus")
                .font(.system(size: UIMetrics.fontMega))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text("No tabs in \(project.name)")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Text("Open a new terminal tab to start working in this project.")
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
                .multilineTextAlignment(.center)
                .frame(maxWidth: UIMetrics.scaled(360))
            Button(action: onCreateTab) {
                HStack(spacing: UIMetrics.spacing4) {
                    Text("New Tab")
                    Text(KeyBindingStore.shared.combo(for: .newTab).displayString)
                        .font(.system(size: UIMetrics.fontFootnote, weight: .medium, design: .rounded))
                        .opacity(0.72)
                }
            }
            .buttonStyle(.borderedProminent)
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

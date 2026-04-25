import SwiftUI

struct PathBreadcrumb: View {
    let path: String

    private var components: [String] {
        path.components(separatedBy: "/").filter { !$0.isEmpty }
    }

    var body: some View {
        HStack(spacing: 3) {
            ForEach(Array(components.enumerated()), id: \.offset) { index, component in
                if index > 0 {
                    Image(systemName: "chevron.right")
                        .font(.system(size: 7, weight: .bold))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
                Text(component)
                    .font(.system(size: 10))
                    .foregroundStyle(index == components.count - 1 ? MuxyTheme.fg : MuxyTheme.fgMuted)
                    .lineLimit(1)
            }
        }
    }
}

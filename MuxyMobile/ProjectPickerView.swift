import MuxyShared
import SwiftUI
import UIKit

struct ProjectPickerView: View {
    @Environment(ConnectionManager.self) private var connection
    @State private var path: [UUID] = []

    var body: some View {
        NavigationStack(path: $path) {
            projectList
                .navigationDestination(for: UUID.self) { _ in
                    WorkspaceContentWrapper()
                }
        }
        .onChange(of: connection.activeProjectID) { _, newValue in
            if let id = newValue, path.last != id {
                path = [id]
            } else if newValue == nil {
                path.removeAll()
            }
        }
        .onChange(of: path) { _, newValue in
            if newValue.isEmpty, connection.activeProjectID != nil {
                connection.activeProjectID = nil
                Task { @MainActor in
                    try? await Task.sleep(for: .milliseconds(400))
                    if connection.activeProjectID == nil {
                        connection.workspace = nil
                    }
                }
            }
        }
        .onAppear {
            if let id = connection.activeProjectID, path.last != id {
                path = [id]
            }
        }
    }

    private var projectList: some View {
        List(connection.projects) { project in
            Button {
                Task { await connection.selectProject(project.id) }
            } label: {
                HStack(spacing: 14) {
                    ProjectIcon(project: project)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(project.name)
                            .font(.body.weight(.medium))
                            .foregroundStyle(themeFg)
                        Text(worktreeSubtitle(for: project.id))
                            .font(.caption)
                            .foregroundStyle(themeFg.opacity(0.6))
                            .lineLimit(1)
                    }
                }
            }
            .listRowBackground(themeFg.opacity(0.06))
        }
        .scrollContentBackground(.hidden)
        .background(themeBg)
        .navigationTitle("Projects")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    connection.disconnect()
                } label: {
                    Image(systemName: "xmark")
                        .foregroundStyle(themeFg)
                }
            }
        }
        .tint(themeFg)
        .refreshable {
            await connection.refreshProjects()
        }
        .background(NavigationBarTitleColorApplier(color: connection.deviceTheme.map { UIColor.fromRGB($0.fg) }))
    }

    private var themeFg: Color {
        connection.deviceTheme?.fgColor ?? .primary
    }

    private var themeBg: Color {
        connection.deviceTheme?.bgColor ?? Color(.systemBackground)
    }

    private var preferredScheme: ColorScheme {
        (connection.deviceTheme?.isDark ?? true) ? .dark : .light
    }

    private func worktreeSubtitle(for projectID: UUID) -> String {
        guard let worktrees = connection.projectWorktrees[projectID],
              let primary = worktrees.first(where: \.isPrimary)
        else { return "default" }
        return primary.branch ?? primary.name
    }
}

extension UIColor {
    static func fromRGB(_ rgb: UInt32) -> UIColor {
        UIColor(
            red: CGFloat((rgb >> 16) & 0xFF) / 255,
            green: CGFloat((rgb >> 8) & 0xFF) / 255,
            blue: CGFloat(rgb & 0xFF) / 255,
            alpha: 1
        )
    }
}

struct NavigationBarTitleColorApplier: UIViewRepresentable {
    let color: UIColor?

    func makeUIView(context _: Context) -> ProbeView {
        ProbeView(color: color)
    }

    func updateUIView(_ view: ProbeView, context _: Context) {
        view.color = color
        view.applyIfPossible()
    }

    final class ProbeView: UIView {
        var color: UIColor?

        init(color: UIColor?) {
            self.color = color
            super.init(frame: .zero)
            isHidden = true
            isUserInteractionEnabled = false
        }

        @available(*, unavailable)
        required init?(coder _: NSCoder) { fatalError() }

        override func didMoveToWindow() {
            super.didMoveToWindow()
            applyIfPossible()
        }

        func applyIfPossible() {
            guard let navBar = findNavigationBar() else { return }
            let appearance = UINavigationBarAppearance()
            appearance.configureWithTransparentBackground()
            if let color {
                appearance.largeTitleTextAttributes = [.foregroundColor: color]
                appearance.titleTextAttributes = [.foregroundColor: color]
            }
            navBar.standardAppearance = appearance
            navBar.scrollEdgeAppearance = appearance
            navBar.compactAppearance = appearance
        }

        private func findNavigationBar() -> UINavigationBar? {
            var responder: UIResponder? = self
            while let current = responder {
                if let vc = current as? UIViewController {
                    return vc.navigationController?.navigationBar
                }
                responder = current.next
            }
            return nil
        }
    }
}


struct ProjectIcon: View {
    let project: ProjectDTO
    var size: CGFloat = 36
    @Environment(ConnectionManager.self) private var connection

    var body: some View {
        if let imageData = connection.projectLogos[project.id],
           let uiImage = UIImage(data: imageData)
        {
            Image(uiImage: uiImage)
                .resizable()
                .aspectRatio(contentMode: .fill)
                .frame(width: size, height: size)
                .clipShape(RoundedRectangle(cornerRadius: size * 0.22))
        } else if let swatch = ProjectIconColor.swatch(for: project.iconColor),
                  let fill = Color(hex: swatch.hex)
        {
            ZStack {
                RoundedRectangle(cornerRadius: size * 0.22)
                    .fill(fill)
                    .frame(width: size, height: size)
                Text(project.name.prefix(1).uppercased())
                    .font(.system(size: size * 0.4, weight: .bold, design: .rounded))
                    .foregroundStyle(swatch.prefersDarkForeground ? Color.black : Color.white)
            }
        } else {
            ZStack {
                RoundedRectangle(cornerRadius: size * 0.22)
                    .fill(.tint.opacity(0.15))
                    .frame(width: size, height: size)
                Text(project.name.prefix(1).uppercased())
                    .font(.system(size: size * 0.4, weight: .bold, design: .rounded))
                    .foregroundStyle(.tint)
            }
        }
    }
}

private extension Color {
    init?(hex: String) {
        guard let rgb = ProjectIconColor.rgb(fromHex: hex) else { return nil }
        self = Color(.sRGB, red: rgb.0, green: rgb.1, blue: rgb.2, opacity: 1)
    }
}

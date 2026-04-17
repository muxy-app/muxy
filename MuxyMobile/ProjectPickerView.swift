import MuxyShared
import SwiftUI

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
                connection.workspace = nil
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
                        Text(worktreeSubtitle(for: project.id))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                }
            }
            .foregroundStyle(.primary)
        }
        .navigationTitle("Projects")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    connection.disconnect()
                } label: {
                    Image(systemName: "xmark")
                }
            }
        }
        .refreshable {
            await connection.refreshProjects()
        }
    }

    private func worktreeSubtitle(for projectID: UUID) -> String {
        guard let worktrees = connection.projectWorktrees[projectID],
              let primary = worktrees.first(where: \.isPrimary)
        else { return "default" }
        return primary.branch ?? primary.name
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

import MuxyShared
import SwiftUI

struct ProjectPickerView: View {
    @Environment(ConnectionManager.self) private var connection

    var body: some View {
        if connection.activeProjectID != nil, connection.workspace != nil {
            WorkspaceView()
        } else {
            projectList
        }
    }

    private var projectList: some View {
        NavigationStack {
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
                        Image(systemName: "xmark.circle.fill")
                            .symbolRenderingMode(.hierarchical)
                    }
                }
            }
            .refreshable {
                await connection.refreshProjects()
            }
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

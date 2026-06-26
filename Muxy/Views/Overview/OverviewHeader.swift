import AppKit
import MuxyShared
import SwiftUI

struct OverviewProjectSection: View {
    let project: Project
    let worktree: Worktree?

    @Environment(ProjectGroupStore.self) private var projectGroupStore

    var body: some View {
        OverviewSection(
            title: "Project",
            storageKey: OverviewSidebarPreferences.projectSectionExpandedKey
        ) {
            VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                HStack(spacing: UIMetrics.spacing3) {
                    icon
                    VStack(alignment: .leading, spacing: UIMetrics.scaled(1)) {
                        Text(project.name)
                            .font(.system(size: UIMetrics.fontEmphasis, weight: .semibold))
                            .foregroundStyle(MuxyTheme.fg)
                            .lineLimit(1)
                            .truncationMode(.tail)
                        if let subtitle {
                            Text(subtitle)
                                .font(.system(size: UIMetrics.fontFootnote))
                                .foregroundStyle(MuxyTheme.fgMuted)
                                .lineLimit(1)
                                .truncationMode(.middle)
                        }
                    }
                    Spacer(minLength: 0)
                }

                pathRow
            }
        }
    }

    private var subtitle: String? {
        if project.isRemote {
            return projectGroupStore.device(for: project)?.displayName ?? "Remote"
        }
        return nil
    }

    private var displayLetter: String {
        String(project.name.prefix(1)).uppercased()
    }

    private var icon: some View {
        ZStack {
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD, style: .continuous)
                .fill(iconBackground)
            if project.isHome {
                Image(systemName: Project.homeIcon)
                    .font(.system(size: UIMetrics.fontTitleLarge, weight: .medium))
                    .foregroundStyle(MuxyTheme.accentForeground)
            } else if let logo {
                Image(nsImage: logo)
                    .resizable()
                    .scaledToFill()
                    .frame(width: UIMetrics.iconXXL, height: UIMetrics.iconXXL)
                    .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD, style: .continuous))
            } else if let iconName = project.icon {
                Image(systemName: iconName)
                    .font(.system(size: UIMetrics.fontTitleLarge, weight: .medium))
                    .foregroundStyle(letterForeground)
            } else {
                Text(displayLetter)
                    .font(.system(size: UIMetrics.fontEmphasis, weight: .bold))
                    .foregroundStyle(letterForeground)
            }
        }
        .frame(width: UIMetrics.iconXXL, height: UIMetrics.iconXXL)
    }

    @ViewBuilder
    private var pathRow: some View {
        let path = worktree?.path ?? project.path
        Button {
            if project.isRemote {
                copyPath(path)
            } else {
                revealInFinder(path)
            }
        } label: {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: project.isRemote ? "doc.on.doc" : "folder")
                    .font(.system(size: UIMetrics.fontXS, weight: .medium))
                    .foregroundStyle(MuxyTheme.fgMuted)
                Text(abbreviatePath(path))
                    .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, UIMetrics.spacing3)
            .padding(.vertical, UIMetrics.spacing2)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        }
        .buttonStyle(.plain)
        .help(project.isRemote ? "Copy path" : "Reveal in Finder")
        .accessibilityLabel(project.isRemote ? "Copy path" : "Reveal in Finder")
    }

    private var iconBackground: AnyShapeStyle {
        if project.isHome { return AnyShapeStyle(MuxyTheme.accent) }
        if logo != nil { return AnyShapeStyle(Color.clear) }
        if let tint = ProjectIconColor.color(for: project.iconColor) {
            return AnyShapeStyle(tint)
        }
        return AnyShapeStyle(MuxyTheme.fg.opacity(0.18))
    }

    private var letterForeground: Color {
        ProjectIconColor.foreground(for: project.iconColor) ?? MuxyTheme.fg
    }

    private var logo: NSImage? {
        guard let filename = project.logo,
              let path = ProjectLogoStorage.safeLogoPath(for: filename)
        else { return nil }
        return NSImage(contentsOfFile: path)
    }

    private func abbreviatePath(_ path: String) -> String {
        let home = NSHomeDirectory()
        guard path.hasPrefix(home) else { return path }
        return "~" + path.dropFirst(home.count)
    }

    private func revealInFinder(_ path: String) {
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
    }

    private func copyPath(_ path: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(path, forType: .string)
    }
}

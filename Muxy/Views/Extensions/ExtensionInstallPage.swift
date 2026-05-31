import SwiftUI

struct ExtensionInstallPage: View {
    let name: String
    let store: ExtensionStore
    let onInstalled: (String) -> Void

    @State private var phase: Phase = .loading
    @State private var isInstalling = false
    @State private var installError: String?

    private enum Phase {
        case loading
        case failed(String)
        case loaded(MarketplaceExtension)
    }

    private var isInstalled: Bool {
        store.statuses.contains { $0.id == name }
    }

    private var installedVersion: String? {
        store.statuses.first { $0.id == name }?.muxyExtension.manifest.version
    }

    var body: some View {
        ScrollView {
            content
                .padding(20)
                .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .task(id: name) { await load() }
    }

    @ViewBuilder
    private var content: some View {
        switch phase {
        case .loading:
            loadingState
        case let .failed(message):
            failedState(message)
        case let .loaded(ext):
            loadedState(ext)
        }
    }

    private var loadingState: some View {
        HStack(spacing: 10) {
            ProgressView().controlSize(.small)
            Text("Loading \(name)…")
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 80)
    }

    private func failedState(_ message: String) -> some View {
        VStack(spacing: 10) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 28))
                .foregroundStyle(MuxyTheme.fgDim)
            Text(message)
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fgMuted)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
            Button {
                Task { await load() }
            } label: {
                Text("Retry")
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.accent)
            }
            .buttonStyle(.plain)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 60)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 10))
    }

    private func loadedState(_ ext: MarketplaceExtension) -> some View {
        VStack(alignment: .leading, spacing: 18) {
            heroBlock(ext)
            if let description = ext.description, !description.isEmpty {
                Text(description)
                    .font(.system(size: 13))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            permissionsBlock(ext)
            safetyNotice
            if let error = installError {
                errorBlock(error)
            }
            installAction(ext)
        }
    }

    private func heroBlock(_ ext: MarketplaceExtension) -> some View {
        HStack(alignment: .top, spacing: 14) {
            ExtensionInstallIcon(urlString: ext.iconURL)
            VStack(alignment: .leading, spacing: 6) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(ext.name)
                        .font(.system(size: 18, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fg)
                    Text("v\(ext.currentVersion)")
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
                if let author = ext.author?.name, !author.isEmpty {
                    Text("by \(author)")
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
                HStack(spacing: 12) {
                    Label("\(ext.downloads)", systemImage: "arrow.down.circle")
                        .font(.system(size: 11))
                        .foregroundStyle(MuxyTheme.fgDim)
                    ForEach(externalLinks(ext), id: \.title) { link in
                        Link(link.title, destination: link.url)
                            .font(.system(size: 11))
                            .foregroundStyle(MuxyTheme.accent)
                    }
                }
            }
            Spacer(minLength: 0)
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 10))
    }

    private func permissionsBlock(_ ext: MarketplaceExtension) -> some View {
        InstallSection(title: "Permissions") {
            if ext.permissions.isEmpty {
                Text("This extension requests no permissions.")
                    .font(.system(size: 11))
                    .foregroundStyle(MuxyTheme.fgDim)
            } else {
                ExtensionInstallPermissionFlow(permissions: ext.permissions)
            }
        }
    }

    private var safetyNotice: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "lock.shield")
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(MuxyTheme.accent)
            Text("No extension can run a command on your computer without you approving each command first.")
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fg)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(MuxyTheme.accentSoft, in: RoundedRectangle(cornerRadius: 8))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(MuxyTheme.accent.opacity(0.3), lineWidth: 1)
        )
    }

    private func errorBlock(_ message: String) -> some View {
        Text(message)
            .font(.system(size: 11))
            .foregroundStyle(MuxyTheme.diffRemoveFg)
            .fixedSize(horizontal: false, vertical: true)
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(MuxyTheme.diffRemoveBg, in: RoundedRectangle(cornerRadius: 8))
    }

    private func installAction(_ ext: MarketplaceExtension) -> some View {
        HStack(spacing: 12) {
            Button {
                Task { await install(ext) }
            } label: {
                HStack(spacing: 6) {
                    if isInstalling {
                        ProgressView().controlSize(.small)
                    }
                    Text(installButtonTitle)
                        .font(.system(size: 13, weight: .semibold))
                }
                .foregroundStyle(.white)
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .background(MuxyTheme.accent, in: RoundedRectangle(cornerRadius: 8))
                .opacity(isInstalling ? 0.7 : 1)
            }
            .buttonStyle(.plain)
            .disabled(isInstalling)
            if isInstalled, let version = installedVersion {
                Text("Installed v\(version)")
                    .font(.system(size: 11))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
            Spacer()
        }
    }

    private var installButtonTitle: String {
        if isInstalling { return isInstalled ? "Reinstalling…" : "Installing…" }
        return isInstalled ? "Reinstall" : "Install"
    }

    private func externalLinks(_ ext: MarketplaceExtension) -> [(title: String, url: URL)] {
        var links: [(String, URL)] = []
        if let repository = ext.repository, let url = URL(string: repository) {
            links.append(("Repository", url))
        }
        if let homepage = ext.homepage, let url = URL(string: homepage) {
            links.append(("Homepage", url))
        }
        return links
    }

    private func load() async {
        phase = .loading
        installError = nil
        do {
            let ext = try await ExtensionMarketplaceService.shared.fetch(name: name)
            phase = .loaded(ext)
        } catch {
            phase = .failed(message(for: error))
        }
    }

    private func install(_ ext: MarketplaceExtension) async {
        isInstalling = true
        installError = nil
        defer { isInstalling = false }
        do {
            let zip = try await ExtensionMarketplaceService.shared.download(ext)
            try await store.install(expectedName: ext.name, zip: zip)
            onInstalled(ext.name)
        } catch {
            installError = message(for: error)
        }
    }

    private func message(for error: Error) -> String {
        (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
    }
}

private struct InstallSection<Content: View>: View {
    let title: String
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .textCase(.uppercase)
                .tracking(0.6)
            content
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(MuxyTheme.surface.opacity(0.5), in: RoundedRectangle(cornerRadius: 10))
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(MuxyTheme.border, lineWidth: 1)
        )
    }
}

private struct ExtensionInstallIcon: View {
    let urlString: String?

    var body: some View {
        Group {
            if let urlString, let url = URL(string: urlString) {
                AsyncImage(url: url) { image in
                    image.resizable().scaledToFit()
                } placeholder: {
                    placeholder
                }
            } else {
                placeholder
            }
        }
        .frame(width: 48, height: 48)
        .background(MuxyTheme.bg, in: RoundedRectangle(cornerRadius: 10))
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(MuxyTheme.border, lineWidth: 1)
        )
    }

    private var placeholder: some View {
        Image(systemName: "puzzlepiece.extension")
            .font(.system(size: 20))
            .foregroundStyle(MuxyTheme.fgDim)
    }
}

private struct ExtensionInstallPermissionFlow: View {
    let permissions: [String]

    var body: some View {
        FlowLayout(spacing: 6, lineSpacing: 6) {
            ForEach(permissions, id: \.self) { permission in
                ExtensionInstallPermissionTag(permission: permission)
            }
        }
    }
}

private struct ExtensionInstallPermissionTag: View {
    let permission: String

    var body: some View {
        let color = tagColor
        HStack(spacing: 4) {
            Circle().fill(color).frame(width: 5, height: 5)
            Text(permission)
                .font(.system(size: 10, weight: .medium, design: .monospaced))
                .foregroundStyle(color)
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 3)
        .background(color.opacity(0.1), in: RoundedRectangle(cornerRadius: 5))
        .overlay(
            RoundedRectangle(cornerRadius: 5)
                .stroke(color.opacity(0.35), lineWidth: 1)
        )
    }

    private var tagColor: Color {
        guard let kind = ExtensionPermission(rawValue: permission)?.kind else {
            return MuxyTheme.fgMuted
        }
        switch kind {
        case .read: return MuxyTheme.warning
        case .write: return MuxyTheme.diffRemoveFg
        case .action: return MuxyTheme.accent
        }
    }
}

private struct FlowLayout: Layout {
    var spacing: CGFloat
    var lineSpacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache _: inout ()) -> CGSize {
        let maxWidth = proposal.width ?? .infinity
        let layout = arrange(subviews: subviews, in: maxWidth)
        return CGSize(width: maxWidth.isFinite ? maxWidth : layout.width, height: layout.height)
    }

    func placeSubviews(in bounds: CGRect, proposal _: ProposedViewSize, subviews: Subviews, cache _: inout ()) {
        let layout = arrange(subviews: subviews, in: bounds.width)
        for placement in layout.placements {
            subviews[placement.index].place(
                at: CGPoint(x: bounds.minX + placement.point.x, y: bounds.minY + placement.point.y),
                proposal: ProposedViewSize(placement.size)
            )
        }
    }

    private struct Placement {
        let index: Int
        let point: CGPoint
        let size: CGSize
    }

    private struct ArrangedLayout {
        let width: CGFloat
        let height: CGFloat
        let placements: [Placement]
    }

    private func arrange(subviews: Subviews, in maxWidth: CGFloat) -> ArrangedLayout {
        var placements: [Placement] = []
        var currentX: CGFloat = 0
        var currentY: CGFloat = 0
        var lineHeight: CGFloat = 0
        var widest: CGFloat = 0
        for index in subviews.indices {
            let size = subviews[index].sizeThatFits(.unspecified)
            if currentX > 0, currentX + size.width > maxWidth {
                currentY += lineHeight + lineSpacing
                currentX = 0
                lineHeight = 0
            }
            placements.append(Placement(index: index, point: CGPoint(x: currentX, y: currentY), size: size))
            currentX += size.width + spacing
            lineHeight = max(lineHeight, size.height)
            widest = max(widest, currentX - spacing)
        }
        return ArrangedLayout(width: widest, height: currentY + lineHeight, placements: placements)
    }
}

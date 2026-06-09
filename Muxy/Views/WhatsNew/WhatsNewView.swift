import SwiftUI

struct WhatsNewView: View {
    let version: String
    let preloadedMarkdown: String?

    @State private var blocks: [ReleaseNotesBlock] = []
    @State private var phase: Phase = .loading

    private enum Phase: Equatable {
        case loading
        case loaded
        case failed
    }

    private var releaseURL: URL {
        let string = "https://github.com/muxy-app/muxy/releases/tag/v\(version)"
        return URL(string: string) ?? HelpLinks.repoURL
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Rectangle()
                .fill(SettingsStyle.border)
                .frame(height: 1)
            content
        }
        .background(SettingsStyle.background)
        .task { await load() }
    }

    private var header: some View {
        HStack(spacing: 12) {
            Image(systemName: "sparkles")
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(SettingsStyle.accent)

            Text("What's New")
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(SettingsStyle.foreground)

            Text("v\(version)")
                .font(.system(size: 12))
                .foregroundStyle(SettingsStyle.mutedForeground)

            Spacer()

            Button {
                NSWorkspace.shared.open(releaseURL)
            } label: {
                Text("View on GitHub")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(SettingsStyle.accent)
            }
            .buttonStyle(.plain)
            .help("Open this release on GitHub")

            Button {
                NSApp.keyWindow?.close()
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .frame(width: 28, height: 28)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Close")
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .frame(height: 56)
        .background(SettingsStyle.background)
    }

    @ViewBuilder
    private var content: some View {
        switch phase {
        case .loading:
            centered { ProgressView().controlSize(.small) }
        case .failed:
            centered {
                VStack(spacing: 12) {
                    Text("Couldn't load the release notes.")
                        .font(.system(size: 12))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                    Button("Retry") { Task { await reload() } }
                }
            }
        case .loaded:
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
                        ReleaseNotesBlockView(block: block)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(20)
                .textSelection(.enabled)
            }
        }
    }

    private func centered(@ViewBuilder _ inner: () -> some View) -> some View {
        VStack { inner() }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func load() async {
        if let preloadedMarkdown {
            blocks = ReleaseNotesMarkdown.parse(preloadedMarkdown)
            phase = .loaded
            return
        }
        await reload()
    }

    private func reload() async {
        phase = .loading
        do {
            let markdown = try await WhatsNewService.fetchReleaseNotes(version: version)
            blocks = ReleaseNotesMarkdown.parse(markdown)
            phase = .loaded
        } catch {
            phase = .failed
        }
    }
}

private struct ReleaseNotesBlockView: View {
    let block: ReleaseNotesBlock

    var body: some View {
        switch block {
        case let .heading(level, spans):
            styledText(spans)
                .font(.system(size: headingSize(for: level), weight: .semibold))
                .foregroundStyle(SettingsStyle.foreground)
                .padding(.top, level <= 2 ? 6 : 2)
        case let .bullet(spans):
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text("•")
                    .font(.system(size: 12))
                    .foregroundStyle(SettingsStyle.mutedForeground)
                styledText(spans)
                    .font(.system(size: 12))
                    .foregroundStyle(SettingsStyle.foreground)
                    .fixedSize(horizontal: false, vertical: true)
            }
        case let .paragraph(spans):
            styledText(spans)
                .font(.system(size: 12))
                .foregroundStyle(SettingsStyle.foreground)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func styledText(_ spans: [ReleaseNotesSpan]) -> Text {
        spans.reduce(Text("")) { partial, span in
            partial + Text(span.text).fontWeight(span.isBold ? .semibold : .regular)
        }
    }

    private func headingSize(for level: Int) -> CGFloat {
        switch level {
        case 1: 16
        case 2: 14
        default: 13
        }
    }
}

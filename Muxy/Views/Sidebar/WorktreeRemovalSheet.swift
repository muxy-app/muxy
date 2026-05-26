import AppKit
import SwiftUI

@MainActor
@Observable
final class WorktreeRemovalController {
    enum Phase: Equatable {
        case running
        case failed(message: String)
    }

    let worktree: Worktree
    private(set) var lines: [WorktreeTeardownOutputLine] = []
    private(set) var phase: Phase = .running

    init(worktree: Worktree) {
        self.worktree = worktree
    }

    func append(_ line: WorktreeTeardownOutputLine) {
        lines.append(line)
    }

    func markFailed(_ error: Error) {
        phase = .failed(message: error.localizedDescription)
    }
}

struct WorktreeRemovalSheet: View {
    @Bindable var controller: WorktreeRemovalController
    let onDismiss: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing5) {
            header
            logView
            footer
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(560), height: UIMetrics.scaled(380))
    }

    private var header: some View {
        HStack(spacing: UIMetrics.spacing3) {
            if case .running = controller.phase {
                ProgressView()
                    .controlSize(.small)
            } else {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
            }
            VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
                Text(headline)
                    .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
                Text(subhead)
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
            Spacer()
        }
    }

    private var headline: String {
        switch controller.phase {
        case .running:
            "Removing worktree \"\(controller.worktree.name)\""
        case .failed:
            "Could not remove worktree \"\(controller.worktree.name)\""
        }
    }

    private var subhead: String {
        switch controller.phase {
        case .running:
            "Running teardown commands."
        case let .failed(message):
            message
        }
    }

    private var logView: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(controller.lines) { line in
                        Text(line.text)
                            .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                            .foregroundStyle(color(for: line.channel))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .textSelection(.enabled)
                            .id(line.id)
                    }
                }
                .padding(UIMetrics.spacing4)
            }
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .onChange(of: controller.lines.count) { _, _ in
                guard let last = controller.lines.last else { return }
                withAnimation(.linear(duration: 0.1)) {
                    proxy.scrollTo(last.id, anchor: .bottom)
                }
            }
        }
    }

    @ViewBuilder
    private var footer: some View {
        if case .failed = controller.phase {
            HStack {
                Spacer()
                Button("Close", action: onDismiss)
                    .keyboardShortcut(.defaultAction)
            }
        }
    }

    private func color(for channel: WorktreeTeardownOutputLine.Channel) -> Color {
        switch channel {
        case .command:
            MuxyTheme.fg
        case .stdout:
            MuxyTheme.fg.opacity(0.85)
        case .stderr:
            MuxyTheme.diffRemoveFg
        case .status:
            MuxyTheme.fgMuted
        }
    }
}

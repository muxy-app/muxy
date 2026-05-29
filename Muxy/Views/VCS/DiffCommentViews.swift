import SwiftUI

enum DiffCommentMetrics {
    static let gutterStripWidth: CGFloat = 44
    static let blockHorizontalInset: CGFloat = 12
}

struct DiffAgentMenuButton: View {
    let disabled: Bool
    let run: (AIAssistantProvider) -> Void

    var body: some View {
        Menu {
            ForEach(AIAssistantProvider.allCases) { provider in
                Button(provider.displayName) { run(provider) }
            }
        } label: {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: "terminal")
                Text("Run by agent")
                Image(systemName: "chevron.down")
                    .font(.system(size: UIMetrics.fontXS, weight: .bold))
            }
            .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
            .foregroundStyle(MuxyTheme.fg)
            .padding(.horizontal, UIMetrics.spacing4)
            .frame(height: UIMetrics.controlMedium)
            .background(MuxyTheme.bg, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
            .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .disabled(disabled)
        .help("Run by agent in a new tab")
    }
}

struct DiffCommentAvatar: View {
    let url: URL?
    let size: CGFloat

    var body: some View {
        Group {
            if let url {
                AsyncImage(url: url) { image in
                    image.resizable().scaledToFill()
                } placeholder: {
                    placeholder
                }
            } else {
                placeholder
            }
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
    }

    private var placeholder: some View {
        Image(systemName: "person.crop.circle.fill")
            .resizable()
            .scaledToFit()
            .foregroundStyle(MuxyTheme.fgDim)
    }
}

struct DiffCommentComposer: View {
    @Bindable var state: DiffViewerTabState
    let target: DiffCommentComposerTarget
    @State private var text = ""
    @FocusState private var focused: Bool

    private var submitTitle: String {
        state.isPR ? "Add comment to PR" : "Add comment"
    }

    private var isEmpty: Bool {
        text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        VStack(spacing: 0) {
            TextField("Add a comment…", text: $text, axis: .vertical)
                .textFieldStyle(.plain)
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(2 ... 6)
                .frame(minHeight: DiffCommentLayout.composerMinFieldHeight, alignment: .top)
                .focused($focused)
                .padding(UIMetrics.spacing3)
                .background(MuxyTheme.bg, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
                .padding(UIMetrics.spacing3)
                .onChange(of: text) { _, newValue in
                    state.composerLineCount = max(1, newValue.split(separator: "\n", omittingEmptySubsequences: false).count)
                }

            Rectangle().fill(MuxyTheme.border).frame(height: 1)

            HStack(spacing: UIMetrics.spacing3) {
                Spacer(minLength: 0)

                textButton("Cancel", filled: false, disabled: false, action: state.cancelComposer)
                    .keyboardShortcut(.cancelAction)

                DiffAgentMenuButton(disabled: isEmpty, run: runByAgent)

                textButton(submitTitle, filled: true, disabled: isEmpty, action: submit)
            }
            .padding(UIMetrics.spacing3)
        }
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusMD).stroke(MuxyTheme.border, lineWidth: 1))
        .onAppear { focused = true }
    }

    private func textButton(_ title: String, filled: Bool, disabled: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title)
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(filled ? Color.white : MuxyTheme.fg)
                .padding(.horizontal, UIMetrics.spacing4)
                .frame(height: UIMetrics.controlMedium)
                .background(filled ? MuxyTheme.accent : MuxyTheme.bg, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(filled ? Color.clear : MuxyTheme.border, lineWidth: 1))
        }
        .buttonStyle(.plain)
        .disabled(disabled)
    }

    private func submit() {
        state.submitComment(body: text)
        text = ""
    }

    private func runByAgent(_ provider: AIAssistantProvider) {
        state.runByAgent(target: target, body: text, provider: provider)
        text = ""
    }
}

struct DiffCommentThread: View {
    @Bindable var state: DiffViewerTabState
    let comment: DiffInlineComment

    var body: some View {
        HStack(alignment: .top, spacing: UIMetrics.spacing3) {
            DiffCommentAvatar(url: state.commentAuthorAvatarURL, size: UIMetrics.scaled(22))

            VStack(alignment: .leading, spacing: UIMetrics.scaled(2)) {
                HStack(spacing: UIMetrics.spacing3) {
                    Text(comment.author)
                        .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fg)
                    statusBadge
                    Spacer(minLength: 0)
                    actions
                }
                Text(comment.body)
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(UIMetrics.spacing3)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusMD).stroke(MuxyTheme.border, lineWidth: 1))
    }

    @ViewBuilder
    private var statusBadge: some View {
        switch comment.submissionState {
        case .posting:
            badge("Posting…", color: MuxyTheme.fgMuted)
        case .posted:
            badge("Posted", color: MuxyTheme.diffAddFg)
        case let .failed(message):
            badge("Failed", color: MuxyTheme.diffRemoveFg).help(message)
        case .outdated:
            badge("Outdated", color: MuxyTheme.fgMuted)
        case .draft:
            EmptyView()
        }
    }

    private func badge(_ title: String, color: Color) -> some View {
        Text(title)
            .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
            .foregroundStyle(color)
            .padding(.horizontal, UIMetrics.scaled(5))
            .padding(.vertical, UIMetrics.scaled(1))
            .background(color.opacity(0.12), in: Capsule())
    }

    @ViewBuilder
    private var actions: some View {
        if comment.submissionState == .draft {
            DiffAgentMenuButton(disabled: false) { provider in
                state.runByAgent(commentID: comment.id, provider: provider)
            }

            Button { state.removeComment(id: comment.id) } label: {
                Image(systemName: "trash")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
            .buttonStyle(.plain)
            .help("Delete comment")
        }
    }
}

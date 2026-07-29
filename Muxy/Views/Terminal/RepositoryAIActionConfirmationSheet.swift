import SwiftUI

struct RepositoryAIActionConfirmationSheet: View {
    let confirmation: RepositoryAIActionConfirmation
    let onCancel: () -> Void
    let onConfirm: (String?) -> Void

    @State private var isAddingPrompt = false
    @State private var additionalPrompt = ""
    @FocusState private var isPromptFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text(L10n.resource(confirmationTitle))
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)

            Text(L10n.resource(confirmationMessage))
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)

            if isAddingPrompt {
                additionalPromptEditor
            }

            HStack(spacing: UIMetrics.spacing3) {
                if !isAddingPrompt {
                    Button(L10n.string("Add Prompt")) {
                        isAddingPrompt = true
                        isPromptFocused = true
                    }
                }
                Spacer(minLength: 0)
                Button(L10n.string("Cancel"), action: onCancel)
                    .keyboardShortcut(.cancelAction)
                Button(L10n.string(key: confirmation.confirmTitle)) {
                    onConfirm(RepositoryAIActionPreferences.normalizedPrompt(additionalPrompt))
                }
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(460))
        .background(MuxyTheme.bg)
    }

    private var additionalPromptEditor: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            HStack {
                Text(L10n.resource("Additional Prompt"))
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                    .foregroundStyle(MuxyTheme.fgMuted)
                Spacer(minLength: 0)
                Text(L10n.resource("\(additionalPrompt.count)/\(RepositoryAIActionPreferences.maximumAdditionalPromptLength)"))
                    .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgDim)
            }

            TextEditor(text: $additionalPrompt)
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fg)
                .scrollContentBackground(.hidden)
                .padding(UIMetrics.spacing3)
                .frame(height: UIMetrics.scaled(140))
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                .overlay {
                    RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                        .stroke(MuxyTheme.border, lineWidth: 1)
                }
                .focused($isPromptFocused)
                .accessibilityLabel(L10n.string("Additional prompt"))
                .onChange(of: additionalPrompt) { _, newValue in
                    guard newValue.count > RepositoryAIActionPreferences.maximumAdditionalPromptLength else { return }
                    additionalPrompt = String(newValue.prefix(RepositoryAIActionPreferences.maximumAdditionalPromptLength))
                }

            Text(L10n.resource("Appended after the configured prompt for this action only."))
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
    }

    private var confirmationTitle: LocalizedStringResource {
        switch confirmation.action {
        case .commit:
            "Commit and push to \"\(confirmation.context.branch)\"?"
        case .createPullRequest:
            "Create a pull request from \"\(confirmation.context.branch)\"?"
        }
    }

    private var confirmationMessage: LocalizedStringResource {
        let providerName = confirmation.providerName
        let branch = confirmation.context.branch
        return switch confirmation.action {
        case .commit:
            "Muxy will stage all changes, ask \(providerName) for a commit message, then commit and push to \"\(branch)\"."
        case .createPullRequest:
            """
            Muxy will stage all changes, ask \(providerName) for a branch name, title, and summary, \
            then create the branch, commit, push, and open the pull request on GitHub.
            """
        }
    }
}

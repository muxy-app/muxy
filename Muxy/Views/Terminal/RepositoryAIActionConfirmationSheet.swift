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
            Text(confirmation.title)
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)

            Text(confirmation.message)
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)

            if isAddingPrompt {
                additionalPromptEditor
            }

            HStack(spacing: UIMetrics.spacing3) {
                if !isAddingPrompt {
                    Button("Add Prompt") {
                        isAddingPrompt = true
                        isPromptFocused = true
                    }
                }
                Spacer(minLength: 0)
                Button("Cancel", action: onCancel)
                    .keyboardShortcut(.cancelAction)
                Button(confirmation.confirmTitle) {
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
                Text("Additional Prompt")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                    .foregroundStyle(MuxyTheme.fgMuted)
                Spacer(minLength: 0)
                Text("\(additionalPrompt.count)/\(RepositoryAIActionPreferences.maximumAdditionalPromptLength)")
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
                .accessibilityLabel("Additional prompt")
                .onChange(of: additionalPrompt) { _, newValue in
                    guard newValue.count > RepositoryAIActionPreferences.maximumAdditionalPromptLength else { return }
                    additionalPrompt = String(newValue.prefix(RepositoryAIActionPreferences.maximumAdditionalPromptLength))
                }

            Text("Appended after the configured prompt for this action only.")
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
    }
}

import SwiftUI

struct CreatePRForm: View {
    struct Context {
        let currentBranch: String
        let defaultBranch: String?
        let localBranches: [String]
        let remoteBranches: [String]
        let isLoadingRemoteBranches: Bool
        let hasStagedChanges: Bool
        let hasUnstagedChanges: Bool
    }

    let context: Context
    let inProgress: Bool
    let errorMessage: String?
    @Binding var draft: VCSTabState.PRFormDraft
    let onLoadRemoteBranches: () -> Void
    let onSubmit: (
        _ baseBranch: String,
        _ title: String,
        _ body: String,
        _ branchStrategy: VCSTabState.PRBranchStrategy,
        _ includeMode: VCSTabState.PRIncludeMode,
        _ draft: Bool
    ) -> Void
    let onCancel: () -> Void
    let onGenerateAI: ((_ baseBranch: String) async throws -> AIPullRequestDraft)?

    @State private var didLoadRemoteBranches = false
    @State private var isGeneratingAI = false
    @State private var aiError: String?
    @State private var aiTask: Task<Void, Never>?
    @FocusState private var titleFocused: Bool

    private var availableBaseBranches: [String] {
        if !context.remoteBranches.isEmpty {
            return context.remoteBranches
        }
        if didLoadRemoteBranches, context.isLoadingRemoteBranches {
            return []
        }
        return context.localBranches
    }

    private var currentBranchSnapshot: String {
        draft.initialCurrentBranch ?? context.currentBranch
    }

    private var trimmedTitle: String {
        draft.title.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var trimmedBranchName: String {
        draft.newBranchName.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var hasAnyChanges: Bool {
        context.hasStagedChanges || context.hasUnstagedChanges
    }

    private var needsNewBranch: Bool {
        !draft.baseBranch.isEmpty && draft.baseBranch == currentBranchSnapshot
    }

    private var includeMode: VCSTabState.PRIncludeMode {
        if !hasAnyChanges { return .none }
        return draft.includeAll ? .all : .stagedOnly
    }

    private var canSubmit: Bool {
        if trimmedTitle.isEmpty { return false }
        if draft.baseBranch.isEmpty { return false }
        if needsNewBranch, trimmedBranchName.isEmpty { return false }
        return true
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing5) {
            header
            titleField
            descriptionField

            if draft.advanced {
                targetBranchField
                if needsNewBranch {
                    newBranchField
                }
                if hasAnyChanges, context.hasStagedChanges, context.hasUnstagedChanges {
                    includeSection
                }
            }

            HStack(spacing: UIMetrics.spacing5) {
                advancedToggle
                if draft.advanced {
                    draftToggle
                }
                Spacer(minLength: 0)
                footerButtons
            }

            if let errorMessage {
                warning(errorMessage)
            }
        }
        .padding(UIMetrics.spacing5)
        .background(MuxyTheme.bg)
        .onAppear(perform: applyDefaults)
        .onChange(of: availableBaseBranches) { _, newList in
            if !draft.baseBranch.isEmpty, !newList.contains(draft.baseBranch) {
                draft.baseBranch = ""
            }
            applyDefaults()
        }
        .onChange(of: draft.title) { _, newValue in
            guard !draft.userEditedBranchName else { return }
            draft.newBranchName = Self.slugify(newValue)
        }
    }

    private var header: some View {
        HStack(spacing: UIMetrics.spacing4) {
            Button(action: onCancel) {
                HStack(spacing: UIMetrics.spacing2) {
                    Image(systemName: "chevron.left")
                        .font(.system(size: UIMetrics.fontCaption, weight: .bold))
                    Text("Back")
                        .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                }
                .foregroundStyle(MuxyTheme.fgMuted)
                .padding(.vertical, UIMetrics.scaled(3))
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Back to commit")

            Image(systemName: "arrow.triangle.pull")
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(MuxyTheme.accent)
            Text("New Pull Request")
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
            Spacer(minLength: 0)
        }
    }

    private var targetBranchField: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            fieldLabel("Target Branch")
            if availableBaseBranches.isEmpty {
                if context.isLoadingRemoteBranches {
                    HStack(spacing: UIMetrics.spacing3) {
                        ProgressView().controlSize(.small)
                        Text("Loading remote branches…")
                            .font(.system(size: UIMetrics.fontFootnote))
                            .foregroundStyle(MuxyTheme.fgMuted)
                    }
                    .padding(.horizontal, UIMetrics.spacing4)
                    .padding(.vertical, UIMetrics.scaled(7))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .themedFieldBackground()
                } else {
                    Text("No branches found.")
                        .font(.system(size: UIMetrics.fontFootnote))
                        .foregroundStyle(MuxyTheme.diffRemoveFg)
                }
            } else {
                Menu {
                    ForEach(availableBaseBranches, id: \.self) { branch in
                        Button(branch) { draft.baseBranch = branch }
                    }
                    Divider()
                    Button {
                        didLoadRemoteBranches = true
                        onLoadRemoteBranches()
                    } label: {
                        if context.isLoadingRemoteBranches {
                            Text("Loading remote branches…")
                        } else if didLoadRemoteBranches || !context.remoteBranches.isEmpty {
                            Text("Refresh remote branches")
                        } else {
                            Text("Load remote branches")
                        }
                    }
                    .disabled(context.isLoadingRemoteBranches)
                } label: {
                    HStack(spacing: UIMetrics.spacing3) {
                        Image(systemName: "arrow.triangle.branch")
                            .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                            .foregroundStyle(MuxyTheme.fgDim)
                        Text(draft.baseBranch.isEmpty ? "Select branch" : draft.baseBranch)
                            .font(.system(size: UIMetrics.fontBody, design: .monospaced))
                            .foregroundStyle(MuxyTheme.fg)
                            .lineLimit(1)
                            .truncationMode(.middle)
                        Spacer(minLength: 4)
                        Image(systemName: "chevron.down")
                            .font(.system(size: UIMetrics.fontXS, weight: .bold))
                            .foregroundStyle(MuxyTheme.fgDim)
                    }
                    .padding(.horizontal, UIMetrics.spacing4)
                    .padding(.vertical, UIMetrics.scaled(7))
                    .contentShape(Rectangle())
                }
                .menuStyle(.button)
                .menuIndicator(.hidden)
                .buttonStyle(.plain)
                .themedFieldBackground()
            }
        }
    }

    private var titleField: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            HStack(spacing: UIMetrics.spacing2) {
                fieldLabel("Title")
                Spacer(minLength: 0)
                if onGenerateAI != nil {
                    aiGenerateButton
                }
            }
            ThemedTextField(
                text: $draft.title,
                placeholder: "Short summary of the change",
                monospaced: false,
                onSubmit: { if canSubmit, !inProgress { submit() } }
            )
            .focused($titleFocused)
            if let aiError {
                Text(aiError)
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private var aiGenerateButton: some View {
        Button {
            if isGeneratingAI {
                cancelAIGeneration()
            } else {
                generateWithAI()
            }
        } label: {
            HStack(spacing: UIMetrics.spacing2) {
                if isGeneratingAI {
                    ProgressView().controlSize(.mini)
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: UIMetrics.fontCaption))
                } else {
                    Image(systemName: "sparkles")
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                }
                Text(isGeneratingAI ? "Cancel" : "Generate with AI")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
            }
            .foregroundStyle(MuxyTheme.accent)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(inProgress)
        .help(isGeneratingAI ? "Cancel generation" : "Generate title and description from the diff")
    }

    private func generateWithAI() {
        guard let onGenerateAI else { return }
        guard !draft.baseBranch.isEmpty else {
            aiError = "Select a target branch first."
            return
        }
        isGeneratingAI = true
        aiError = nil
        let base = draft.baseBranch
        aiTask?.cancel()
        aiTask = Task { @MainActor in
            do {
                let aiDraft = try await onGenerateAI(base)
                guard !Task.isCancelled else { return }
                draft.title = aiDraft.title
                draft.body = aiDraft.body
                isGeneratingAI = false
                aiTask = nil
            } catch is CancellationError {
                isGeneratingAI = false
                aiTask = nil
            } catch {
                guard !Task.isCancelled else { return }
                aiError = error.localizedDescription
                isGeneratingAI = false
                aiTask = nil
            }
        }
    }

    private func cancelAIGeneration() {
        aiTask?.cancel()
        aiTask = nil
        isGeneratingAI = false
    }

    private var descriptionField: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            fieldLabel("Description")
            TextEditor(text: $draft.body)
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fg)
                .scrollContentBackground(.hidden)
                .padding(.horizontal, UIMetrics.spacing2)
                .padding(.vertical, UIMetrics.scaled(3))
                .frame(height: UIMetrics.scaled(100))
                .themedFieldBackground()
        }
    }

    private var newBranchField: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            fieldLabel("New Branch")
            ThemedTextField(
                text: $draft.newBranchName,
                placeholder: "branch-name",
                monospaced: true
            )
            .onChange(of: draft.newBranchName) { _, newValue in
                guard !draft.userEditedBranchName else { return }
                if newValue != Self.slugify(draft.title) {
                    draft.userEditedBranchName = true
                }
            }
            Text("A new branch will be created from \(currentBranchSnapshot) for this pull request.")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgDim)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var includeSection: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
            fieldLabel("Include")
            VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
                includeRadio(label: "All changes (staged + unstaged)", value: true)
                includeRadio(label: "Only staged changes", value: false)
            }
        }
    }

    private func includeRadio(label: String, value: Bool) -> some View {
        Button {
            draft.includeAll = value
        } label: {
            HStack(spacing: UIMetrics.spacing3) {
                Image(systemName: draft.includeAll == value ? "largecircle.fill.circle" : "circle")
                    .font(.system(size: UIMetrics.fontBody))
                    .foregroundStyle(draft.includeAll == value ? MuxyTheme.accent : MuxyTheme.fgDim)
                Text(label)
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fg)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private var advancedToggle: some View {
        Button {
            draft.advanced.toggle()
        } label: {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: draft.advanced ? "chevron.up" : "chevron.down")
                    .font(.system(size: UIMetrics.fontXS, weight: .bold))
                Text("Advanced")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
            }
            .foregroundStyle(MuxyTheme.fgMuted)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help("Show target branch, include and draft options")
    }

    private var draftToggle: some View {
        Toggle(isOn: $draft.draft) {
            Text("Create as draft")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fg)
        }
        .toggleStyle(.checkbox)
    }

    private func warning(_ text: String) -> some View {
        Text(text)
            .font(.system(size: UIMetrics.fontFootnote))
            .foregroundStyle(MuxyTheme.diffRemoveFg)
            .fixedSize(horizontal: false, vertical: true)
    }

    private var footerButtons: some View {
        HStack(spacing: UIMetrics.spacing3) {
            Button(action: onCancel) {
                Text("Cancel")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                    .padding(.horizontal, UIMetrics.spacing6)
                    .padding(.vertical, UIMetrics.spacing3)
                    .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
                    .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusMD).stroke(MuxyTheme.border, lineWidth: 1))
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(inProgress)

            let submitEnabled = canSubmit && !inProgress
            Button(action: submit) {
                HStack(spacing: UIMetrics.spacing2) {
                    if inProgress {
                        ProgressView().controlSize(.mini)
                    }
                    Text("Create PR")
                        .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                }
                .foregroundStyle(submitEnabled ? MuxyTheme.bg : MuxyTheme.fgDim)
                .padding(.horizontal, UIMetrics.spacing6)
                .padding(.vertical, UIMetrics.spacing3)
                .background(
                    submitEnabled ? MuxyTheme.accent : MuxyTheme.surface,
                    in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                )
                .overlay(
                    RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                        .stroke(MuxyTheme.border, lineWidth: submitEnabled ? 0 : 1)
                )
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .keyboardShortcut(.defaultAction)
            .disabled(!submitEnabled)
        }
    }

    private func fieldLabel(_ text: String) -> some View {
        Text(text)
            .font(.system(size: UIMetrics.fontFootnote))
            .foregroundStyle(MuxyTheme.fgMuted)
    }

    private func applyDefaults() {
        if draft.initialCurrentBranch == nil {
            draft.initialCurrentBranch = context.currentBranch
        }
        if draft.baseBranch.isEmpty {
            draft.baseBranch = context.defaultBranch
                ?? availableBaseBranches.first(where: { $0 != currentBranchSnapshot })
                ?? availableBaseBranches.first
                ?? ""
        }
        titleFocused = true
    }

    private func submit() {
        let strategy: VCSTabState.PRBranchStrategy = needsNewBranch
            ? .createNew(name: trimmedBranchName)
            : .useCurrent
        onSubmit(draft.baseBranch, trimmedTitle, draft.body, strategy, includeMode, draft.draft)
    }

    private static func slugify(_ title: String) -> String {
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "._-"))
        let scalars = title.lowercased().unicodeScalars.map { allowed.contains($0) ? Character($0) : "-" }
        let collapsed = String(scalars)
            .split(separator: "-", omittingEmptySubsequences: true)
            .joined(separator: "-")
        return String(collapsed.prefix(20))
    }
}

private struct ThemedTextField: View {
    @Binding var text: String
    let placeholder: String
    var monospaced: Bool = false
    var onSubmit: (() -> Void)?

    var body: some View {
        TextField(placeholder, text: $text)
            .textFieldStyle(.plain)
            .font(.system(size: UIMetrics.fontBody, design: monospaced ? .monospaced : .default))
            .foregroundStyle(MuxyTheme.fg)
            .padding(.horizontal, UIMetrics.spacing4)
            .padding(.vertical, UIMetrics.scaled(7))
            .frame(maxWidth: .infinity, alignment: .leading)
            .themedFieldBackground()
            .onSubmit { onSubmit?() }
    }
}

private extension View {
    func themedFieldBackground() -> some View {
        background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusMD).stroke(MuxyTheme.border, lineWidth: 1))
    }
}

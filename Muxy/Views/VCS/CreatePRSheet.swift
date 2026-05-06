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

    @State private var didLoadRemoteBranches = false

    private var availableBaseBranches: [String] {
        if !context.remoteBranches.isEmpty {
            return context.remoteBranches
        }
        if didLoadRemoteBranches, context.isLoadingRemoteBranches {
            return []
        }
        return context.localBranches
    }

    @State private var baseBranch: String = ""
    @State private var title: String = ""
    @State private var bodyText: String = ""
    @State private var newBranchName: String = ""
    @State private var userEditedBranchName = false
    @State private var includeAll = true
    @State private var draft = false
    @State private var didApplyDefaults = false
    @State private var initialCurrentBranch: String?
    @State private var advanced = false
    @FocusState private var titleFocused: Bool

    private var currentBranchSnapshot: String {
        initialCurrentBranch ?? context.currentBranch
    }

    private var trimmedTitle: String {
        title.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var trimmedBranchName: String {
        newBranchName.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var hasAnyChanges: Bool {
        context.hasStagedChanges || context.hasUnstagedChanges
    }

    private var needsNewBranch: Bool {
        !baseBranch.isEmpty && baseBranch == currentBranchSnapshot
    }

    private var includeMode: VCSTabState.PRIncludeMode {
        if !hasAnyChanges { return .none }
        return includeAll ? .all : .stagedOnly
    }

    private var canSubmit: Bool {
        if trimmedTitle.isEmpty { return false }
        if baseBranch.isEmpty { return false }
        if needsNewBranch, trimmedBranchName.isEmpty { return false }
        return true
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing5) {
            header
            titleField
            descriptionField

            if advanced {
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
                if advanced {
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
            if !baseBranch.isEmpty, !newList.contains(baseBranch) {
                baseBranch = ""
            }
            applyDefaults()
        }
        .onChange(of: title) { _, newValue in
            guard !userEditedBranchName else { return }
            newBranchName = Self.slugify(newValue)
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
                        Button(branch) { baseBranch = branch }
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
                        Text(baseBranch.isEmpty ? "Select branch" : baseBranch)
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
            fieldLabel("Title")
            ThemedTextField(
                text: $title,
                placeholder: "Short summary of the change",
                monospaced: false,
                onSubmit: { if canSubmit, !inProgress { submit() } }
            )
            .focused($titleFocused)
        }
    }

    private var descriptionField: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            fieldLabel("Description")
            TextEditor(text: $bodyText)
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
                text: $newBranchName,
                placeholder: "branch-name",
                monospaced: true
            )
            .onChange(of: newBranchName) { _, newValue in
                guard !userEditedBranchName else { return }
                if newValue != Self.slugify(title) {
                    userEditedBranchName = true
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
            includeAll = value
        } label: {
            HStack(spacing: UIMetrics.spacing3) {
                Image(systemName: includeAll == value ? "largecircle.fill.circle" : "circle")
                    .font(.system(size: UIMetrics.fontBody))
                    .foregroundStyle(includeAll == value ? MuxyTheme.accent : MuxyTheme.fgDim)
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
            advanced.toggle()
        } label: {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: advanced ? "chevron.up" : "chevron.down")
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
        Toggle(isOn: $draft) {
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
        if initialCurrentBranch == nil {
            initialCurrentBranch = context.currentBranch
        }
        if baseBranch.isEmpty {
            baseBranch = context.defaultBranch
                ?? availableBaseBranches.first(where: { $0 != currentBranchSnapshot })
                ?? availableBaseBranches.first
                ?? ""
        }
        if !didApplyDefaults {
            includeAll = true
            didApplyDefaults = true
        }
        titleFocused = true
    }

    private func submit() {
        let strategy: VCSTabState.PRBranchStrategy = needsNewBranch
            ? .createNew(name: trimmedBranchName)
            : .useCurrent
        onSubmit(baseBranch, trimmedTitle, bodyText, strategy, includeMode, draft)
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

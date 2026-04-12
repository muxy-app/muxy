import SwiftUI

struct CreatePRSheet: View {
    struct Context {
        let currentBranch: String
        let defaultBranch: String?
        let availableBaseBranches: [String]
        let isLoadingBranches: Bool
        let hasStagedChanges: Bool
        let hasUnstagedChanges: Bool
    }

    let context: Context
    let inProgress: Bool
    let errorMessage: String?
    let onSubmit: (
        _ baseBranch: String,
        _ title: String,
        _ body: String,
        _ branchStrategy: VCSTabState.PRBranchStrategy,
        _ includeMode: VCSTabState.PRIncludeMode,
        _ draft: Bool
    ) -> Void
    let onCancel: () -> Void

    @State private var baseBranch: String = ""
    @State private var title: String = ""
    @State private var bodyText: String = ""
    @State private var newBranchName: String = ""
    @State private var userEditedBranchName = false
    @State private var isProgrammaticBranchNameChange = false
    @State private var includeAll = true
    @State private var draft = false
    @State private var didApplyDefaults = false
    @State private var initialCurrentBranch: String?
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
        VStack(alignment: .leading, spacing: 14) {
            header
            targetBranchField
            titleField
            descriptionField

            if needsNewBranch {
                newBranchField
            }

            if hasAnyChanges, context.hasStagedChanges, context.hasUnstagedChanges {
                includeSection
            }

            draftToggle

            if let errorMessage {
                warning(errorMessage)
            }

            footer
        }
        .padding(20)
        .frame(width: 500)
        .onAppear(perform: applyDefaults)
        .onChange(of: context.availableBaseBranches) { _, _ in applyDefaults() }
        .onChange(of: title) { _, newValue in
            guard !userEditedBranchName else { return }
            isProgrammaticBranchNameChange = true
            newBranchName = Self.slugify(newValue)
        }
    }

    private var header: some View {
        HStack(spacing: 8) {
            Image(systemName: "arrow.triangle.pull")
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(MuxyTheme.accent)
            Text("Create Pull Request")
                .font(.system(size: 14, weight: .semibold))
            Spacer(minLength: 0)
        }
    }

    private var targetBranchField: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Target Branch")
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgMuted)
            if context.isLoadingBranches, context.availableBaseBranches.isEmpty {
                HStack(spacing: 6) {
                    ProgressView().controlSize(.small)
                    Text("Loading remote branches…")
                        .font(.system(size: 11))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
            } else if context.availableBaseBranches.isEmpty {
                Text("No remote branches found. Push at least one branch to origin first.")
                    .font(.system(size: 11))
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
            } else {
                Picker("", selection: $baseBranch) {
                    ForEach(context.availableBaseBranches, id: \.self) { branch in
                        Text(branch).tag(branch)
                    }
                }
                .labelsHidden()
            }
        }
    }

    private var titleField: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Title")
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgMuted)
            TextField("Short summary of the change", text: $title)
                .textFieldStyle(.roundedBorder)
                .focused($titleFocused)
                .onSubmit { if canSubmit, !inProgress { submit() } }
        }
    }

    private var descriptionField: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Description")
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgMuted)
            TextEditor(text: $bodyText)
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fg)
                .scrollContentBackground(.hidden)
                .padding(6)
                .frame(minHeight: 100, maxHeight: 160)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 6))
                .overlay(RoundedRectangle(cornerRadius: 6).stroke(MuxyTheme.border, lineWidth: 1))
        }
    }

    private var newBranchField: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("New Branch")
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgMuted)
            TextField("branch-name", text: $newBranchName)
                .textFieldStyle(.roundedBorder)
                .font(.system(size: 11, design: .monospaced))
                .onChange(of: newBranchName) { _, _ in
                    if isProgrammaticBranchNameChange {
                        isProgrammaticBranchNameChange = false
                        return
                    }
                    userEditedBranchName = true
                }
            Text("A new branch will be created from \(currentBranchSnapshot) for this pull request.")
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgDim)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var includeSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Include")
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgMuted)
            Picker("", selection: $includeAll) {
                Text("All changes (staged + unstaged)").tag(true)
                Text("Only staged changes").tag(false)
            }
            .pickerStyle(.radioGroup)
            .labelsHidden()
        }
    }

    private var draftToggle: some View {
        Toggle(isOn: $draft) {
            Text("Create as draft")
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fg)
        }
        .toggleStyle(.checkbox)
    }

    private func warning(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 11))
            .foregroundStyle(MuxyTheme.diffRemoveFg)
            .fixedSize(horizontal: false, vertical: true)
    }

    private var footer: some View {
        HStack {
            Spacer()
            Button("Cancel", action: onCancel)
                .keyboardShortcut(.cancelAction)
                .disabled(inProgress)
            Button {
                submit()
            } label: {
                HStack(spacing: 4) {
                    if inProgress {
                        ProgressView().controlSize(.mini)
                    }
                    Text("Create PR")
                }
            }
            .keyboardShortcut(.defaultAction)
            .disabled(!canSubmit || inProgress)
        }
    }

    private func applyDefaults() {
        if initialCurrentBranch == nil {
            initialCurrentBranch = context.currentBranch
        }
        if baseBranch.isEmpty {
            baseBranch = context.defaultBranch
                ?? context.availableBaseBranches.first(where: { $0 != currentBranchSnapshot })
                ?? context.availableBaseBranches.first
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

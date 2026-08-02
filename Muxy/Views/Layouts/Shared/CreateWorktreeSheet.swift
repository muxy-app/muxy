import AppKit
import SwiftUI

enum CreateWorktreeResult {
    case created(Worktree, runSetup: Bool)
    case cancelled
}

struct WorktreeBranchLoadState: Equatable {
    var branches: [String] = []
    var selectedExistingBranch = ""
    var selectedBaseBranch = ""
    private(set) var isLoading = true

    mutating func beginLoading() {
        isLoading = true
    }

    mutating func finishLoading(branches: [String], defaultBranch: String?) {
        self.branches = branches
        if selectedExistingBranch.isEmpty {
            selectedExistingBranch = branches.first ?? ""
        }
        if selectedBaseBranch.isEmpty {
            if let defaultBranch, branches.contains(defaultBranch) {
                selectedBaseBranch = defaultBranch
            } else {
                selectedBaseBranch = branches.first ?? ""
            }
        }
        isLoading = false
    }

    mutating func failLoading() {
        isLoading = false
    }
}

private struct WorktreeBranchPicker: View {
    private struct BranchOption: Identifiable {
        let name: String
        var id: String { name }
    }

    let label: String
    let branches: [String]
    let isLoading: Bool
    @Binding var selection: String
    @State private var isPresented = false

    private var options: [BranchOption] {
        branches.map(BranchOption.init)
    }

    var body: some View {
        if isLoading {
            loadingField
        } else {
            selectionButton
        }
    }

    private var loadingField: some View {
        HStack(spacing: UIMetrics.spacing3) {
            ProgressView().controlSize(.small)
            Text(L10n.resource("Loading branches…"))
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
        .frame(maxWidth: .infinity, minHeight: UIMetrics.controlMedium, alignment: .leading)
        .padding(.horizontal, UIMetrics.spacing3)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(label)
        .accessibilityValue(L10n.string("Loading branches…"))
    }

    private var selectionButton: some View {
        Button {
            isPresented = true
        } label: {
            HStack(spacing: UIMetrics.spacing3) {
                if selection.isEmpty {
                    Text(L10n.resource("No branches"))
                        .foregroundStyle(MuxyTheme.fgDim)
                } else {
                    Text(selection)
                        .foregroundStyle(MuxyTheme.fg)
                }
                Spacer(minLength: UIMetrics.spacing3)
                Image(systemName: "chevron.down")
                    .font(.system(size: UIMetrics.fontMicro, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgDim)
            }
            .font(.system(size: UIMetrics.fontFootnote))
            .padding(.horizontal, UIMetrics.spacing3)
            .frame(maxWidth: .infinity, minHeight: UIMetrics.controlMedium, alignment: .leading)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(branches.isEmpty)
        .accessibilityLabel(label)
        .accessibilityValue(selection.isEmpty ? L10n.string("No branches") : selection)
        .popover(isPresented: $isPresented, arrowEdge: .bottom) {
            PopoverPicker(
                items: options,
                filterKey: \.name,
                searchPlaceholder: L10n.string("Search branches…"),
                emptyLabel: L10n.string("No branches"),
                onSelect: select,
                row: { option, isHighlighted in
                    HStack(spacing: UIMetrics.spacing3) {
                        Text(option.name)
                            .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))
                            .lineLimit(1)
                            .truncationMode(.middle)
                        Spacer(minLength: UIMetrics.spacing3)
                        if option.name == selection {
                            Image(systemName: "checkmark")
                                .font(.system(size: UIMetrics.fontXS, weight: .bold))
                                .foregroundStyle(MuxyTheme.accent)
                        }
                    }
                    .foregroundStyle(MuxyTheme.fg)
                    .padding(.horizontal, UIMetrics.spacing5)
                    .padding(.vertical, UIMetrics.spacing3)
                    .background(isHighlighted ? MuxyTheme.surface : .clear)
                }
            )
        }
    }

    private func select(_ option: BranchOption) {
        selection = option.name
        isPresented = false
    }
}

struct CreateWorktreeSheet: View {
    let project: Project
    let onFinish: (CreateWorktreeResult) -> Void

    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectStore.self) private var projectStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @AppStorage(GeneralSettingsKeys.defaultWorktreePathTemplate)
    private var defaultWorktreePathTemplate = ""
    @AppStorage(GeneralSettingsKeys.defaultWorktreeParentPath)
    private var defaultWorktreeParentPath = ""
    @State private var name: String = ""
    @State private var branchName: String = ""
    @State private var branchNameEdited = false
    @State private var createNewBranch = true
    @State private var localLocationSelection = WorktreeLocationSelection()
    @State private var branchLoadState = WorktreeBranchLoadState()
    @State private var setupCommands: [String] = []
    @State private var runSetup = false
    @State private var inProgress = false
    @State private var errorMessage: String?
    @State private var remotePath: String = ""
    @State private var remotePathEdited = false

    private var workspaceContext: WorkspaceContext? {
        projectGroupStore.resolvedWorkspaceContext(for: project)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text(L10n.resource("New Worktree"))
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                Text(L10n.resource("Name")).font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                TextField(L10n.string("feature-x"), text: $name)
                    .textFieldStyle(.roundedBorder)
            }

            SegmentedPicker(
                selection: $createNewBranch,
                options: [
                    (true, L10n.string("Create new branch")),
                    (false, L10n.string("Use existing branch")),
                ]
            )

            if createNewBranch {
                VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                    Text(L10n.resource("Branch Name")).font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                    TextField(L10n.string("feature-x"), text: $branchName)
                        .textFieldStyle(.roundedBorder)
                        .onChange(of: branchName) { _, newValue in
                            branchNameEdited = newValue != name
                        }
                }
                VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                    Text(L10n.resource("Base Branch")).font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                    WorktreeBranchPicker(
                        label: L10n.string("Base Branch"),
                        branches: branchLoadState.branches,
                        isLoading: branchLoadState.isLoading,
                        selection: $branchLoadState.selectedBaseBranch
                    )
                }
            } else {
                VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                    Text(L10n.resource("Branch")).font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                    WorktreeBranchPicker(
                        label: L10n.string("Branch"),
                        branches: branchLoadState.branches,
                        isLoading: branchLoadState.isLoading,
                        selection: $branchLoadState.selectedExistingBranch
                    )
                }
            }

            locationSection

            if setupCommands.isEmpty {
                setupCommandsGuideSection
            } else {
                setupCommandsSection
            }

            if let errorMessage {
                Text(errorMessage)
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
                    .fixedSize(horizontal: false, vertical: true)
            }

            HStack {
                Spacer()
                Button(L10n.string("Cancel")) { onFinish(.cancelled) }
                    .keyboardShortcut(.cancelAction)
                Button(L10n.string("Create")) { Task { await create() } }
                    .keyboardShortcut(.defaultAction)
                    .disabled(!canCreate || inProgress)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(460))
        .task {
            loadLocation()
            await loadBranches()
            loadSetupCommands()
        }
        .onChange(of: name) { _, newValue in
            syncRemotePath()
            guard createNewBranch, !branchNameEdited else { return }
            branchName = newValue
        }
        .onChange(of: createNewBranch) { _, isCreatingNewBranch in
            guard isCreatingNewBranch, !branchNameEdited else { return }
            branchName = name
        }
    }

    private var locationSection: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
            Text(L10n.resource("Location")).font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
            if project.isRemote {
                remoteLocationField
            } else {
                localLocationRow
            }
        }
    }

    private var remoteLocationField: some View {
        TextField(L10n.string("~/.muxy-worktrees/<name>"), text: $remotePath)
            .textFieldStyle(.roundedBorder)
            .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))
            .onChange(of: remotePath) { _, newValue in
                remotePathEdited = newValue != worktreeDirectoryPath
            }
    }

    private var localLocationRow: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
            SegmentedPicker(
                selection: localLocationMode,
                options: [
                    (.defaultLocation, L10n.string("Default")),
                    (.pathTemplate, L10n.string("Template")),
                    (.parentFolder, L10n.string("Folder")),
                ]
            )

            switch localLocationSelection.mode {
            case .defaultLocation:
                Text(defaultLocationDescription)
                    .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgMuted)
            case .pathTemplate:
                TextField(WorktreeLocationResolver.suggestedPathTemplate, text: localLocationText)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))
            case .parentFolder:
                HStack(spacing: UIMetrics.spacing4) {
                    TextField(L10n.string("/path/to/worktrees"), text: localLocationText)
                        .textFieldStyle(.roundedBorder)
                        .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))

                    Button(L10n.string("Choose Folder...")) {
                        chooseParentDirectory()
                    }
                    .fixedSize(horizontal: true, vertical: false)
                }
            }

            if let message = localLocationValidationMessage {
                Text(message)
                    .font(.system(size: UIMetrics.fontCaption))
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
            } else {
                Text(worktreeDirectoryPath)
                    .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .textSelection(.enabled)
            }

            Text(L10n.resource("Templates must include {branch}. Relative paths start from the project folder."))
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
    }

    private var setupCommandsSection: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing4) {
            HStack(spacing: UIMetrics.spacing3) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: UIMetrics.fontCaption))
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
                Text(L10n.resource("Setup commands from .muxy/worktree.json"))
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
            }
            Text(L10n.resource("These commands will run in the new worktree's terminal. Only enable this if you trust this repository."))
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
            VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
                ForEach(setupCommands, id: \.self) { command in
                    Text(command)
                        .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fg)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .padding(UIMetrics.spacing4)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            Toggle(L10n.string("Run these commands after creating the worktree"), isOn: $runSetup)
                .font(.system(size: UIMetrics.fontFootnote))
        }
        .padding(UIMetrics.spacing5)
        .background(MuxyTheme.hover, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
    }

    private var setupCommandsGuideSection: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing4) {
            HStack(spacing: UIMetrics.spacing3) {
                Image(systemName: "info.circle")
                    .font(.system(size: UIMetrics.fontCaption))
                    .foregroundStyle(MuxyTheme.fgDim)
                Text(L10n.resource("Optional setup commands"))
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
            }
            Text(L10n.resource("To run setup commands after creating a worktree, add .muxy/worktree.json in this repository."))
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
            Text(L10n.resource("\(project.path)/.muxy/worktree.json"))
                .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                .foregroundStyle(MuxyTheme.fg)
                .textSelection(.enabled)
            Text(L10n.resource("{\n  \"setup\": [\n    \"pnpm install\",\n    \"pnpm dev\"\n  ]\n}"))
                .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                .foregroundStyle(MuxyTheme.fg)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(UIMetrics.spacing4)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
        }
        .padding(UIMetrics.spacing5)
        .background(MuxyTheme.hover, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
    }

    private func loadSetupCommands() {
        guard !project.isRemote else {
            setupCommands = []
            return
        }
        guard let config = WorktreeConfig.load(fromProjectPath: project.path) else {
            setupCommands = []
            return
        }
        setupCommands = config.setup.map(\.command).filter { !$0.isEmpty }
    }

    private func loadLocation() {
        guard !project.isRemote else {
            syncRemotePath()
            return
        }
        if let template = WorktreeLocationResolver.normalizedLocation(project.preferredWorktreePathTemplate) {
            localLocationSelection = WorktreeLocationSelection(pathTemplate: template)
            return
        }
        guard let path = WorktreeLocationResolver.normalizedLocation(project.preferredWorktreeParentPath) else { return }
        localLocationSelection = WorktreeLocationSelection(parentPath: path)
    }

    private func syncRemotePath() {
        guard project.isRemote, !remotePathEdited else { return }
        remotePath = worktreeDirectoryPath
    }

    private var resolvedProject: Project {
        var resolved = project
        resolved.preferredWorktreePathTemplate = localLocationSelection.selectedPathTemplate
        resolved.preferredWorktreeParentPath = localLocationSelection.selectedParentPath
        return resolved
    }

    private var localLocationMode: Binding<WorktreeLocationMode> {
        Binding(
            get: { localLocationSelection.mode },
            set: { mode in
                localLocationSelection.select(mode)
            }
        )
    }

    private var localLocationText: Binding<String> {
        Binding(
            get: { localLocationSelection.value },
            set: { localLocationSelection.value = $0 }
        )
    }

    private var defaultLocationDescription: String {
        if let template = WorktreeLocationResolver.normalizedLocation(defaultWorktreePathTemplate) {
            return "Global template: \(template)"
        }
        if let folder = WorktreeLocationResolver.normalizedLocation(defaultWorktreeParentPath) {
            return "Global folder: \(folder)"
        }
        return "Muxy App Support"
    }

    private var localLocationValidationMessage: String? {
        do {
            try validateLocalLocationSelection()
            _ = try resolvedLocalWorktreeDirectory(slug: displaySlug, branch: displayBranch)
            return nil
        } catch {
            return error.localizedDescription
        }
    }

    private var displaySlug: String {
        let trimmed = name.trimmingCharacters(in: .whitespaces)
        return trimmed.isEmpty ? "name" : WorktreeLocationResolver.slug(from: trimmed)
    }

    private var displayBranch: String {
        let branch = createNewBranch ? branchName : branchLoadState.selectedExistingBranch
        let trimmed = branch.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? "branch" : trimmed
    }

    private var worktreeDirectoryPath: String {
        guard !project.isRemote else {
            return WorktreeLocationResolver.remoteWorktreeDirectory(for: project, slug: displaySlug)
        }
        return (try? resolvedLocalWorktreeDirectory(slug: displaySlug, branch: displayBranch)) ?? ""
    }

    private func chooseParentDirectory() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.message = L10n.string("Select where new worktrees for this project should be created")
        let initialPath = worktreeDirectoryPath.isEmpty ? project.path : worktreeDirectoryPath
        panel.directoryURL = URL(fileURLWithPath: initialPath, isDirectory: true).deletingLastPathComponent()
        guard panel.runModal() == .OK, let url = panel.url else { return }
        localLocationSelection.select(.parentFolder)
        localLocationSelection.value = url.path
    }

    private var canCreate: Bool {
        guard workspaceContext != nil else { return false }
        guard !branchLoadState.isLoading else { return false }
        guard !name.trimmingCharacters(in: .whitespaces).isEmpty else { return false }
        if project.isRemote, remotePath.trimmingCharacters(in: .whitespaces).isEmpty {
            return false
        }
        if !project.isRemote, localLocationValidationMessage != nil {
            return false
        }
        if createNewBranch {
            return !branchName.trimmingCharacters(in: .whitespaces).isEmpty
        }
        return !branchLoadState.selectedExistingBranch.isEmpty
    }

    @MainActor
    private func loadBranches() async {
        branchLoadState.beginLoading()
        guard let workspaceContext else {
            branchLoadState.failLoading()
            errorMessage = "The remote context for \(project.name) is unavailable."
            return
        }
        let gitRepository = GitRepositoryService(context: workspaceContext)
        do {
            async let branchesValue = gitRepository.listBranches(repoPath: project.path)
            async let defaultValue = gitRepository.defaultBranch(repoPath: project.path)
            let branches = try await branchesValue
            let resolvedDefault = await defaultValue
            branchLoadState.finishLoading(branches: branches, defaultBranch: resolvedDefault)
        } catch {
            branchLoadState.failLoading()
            errorMessage = error.localizedDescription
        }
    }

    @MainActor
    private func create() async {
        guard let workspaceContext else {
            errorMessage = "The remote context for \(project.name) is unavailable."
            return
        }
        inProgress = true
        errorMessage = nil
        let trimmedName = name.trimmingCharacters(in: .whitespaces)
        let branch = createNewBranch
            ? branchName.trimmingCharacters(in: .whitespaces)
            : branchLoadState.selectedExistingBranch

        let slug = WorktreeLocationResolver.slug(from: trimmedName)
        let worktreeDirectory: String
        do {
            try validateLocalLocationSelection()
            worktreeDirectory = try resolvedWorktreeDirectory(slug: slug, branch: branch)
        } catch {
            inProgress = false
            errorMessage = error.localizedDescription
            return
        }

        if await workspaceContext.fileOps.exists(at: worktreeDirectory) {
            inProgress = false
            errorMessage = "A worktree with this name already exists on disk."
            return
        }

        let trimmedBase = branchLoadState.selectedBaseBranch.trimmingCharacters(in: .whitespaces)
        let baseBranch: String? = createNewBranch && !trimmedBase.isEmpty ? trimmedBase : nil

        let request = WorktreeCreationRequest(
            name: trimmedName,
            path: worktreeDirectory,
            branch: branch,
            createBranch: createNewBranch,
            baseBranch: baseBranch
        )

        do {
            let worktree = try await worktreeStore.createWorktree(
                project: project,
                request: request,
                context: workspaceContext
            )
            if !project.isRemote {
                try projectStore.setPreferredWorktreeLocation(
                    id: project.id,
                    pathTemplate: localLocationSelection.selectedPathTemplate,
                    parentPath: localLocationSelection.selectedParentPath
                )
            }
            inProgress = false
            onFinish(.created(worktree, runSetup: runSetup))
        } catch {
            inProgress = false
            errorMessage = error.localizedDescription
        }
    }

    private func resolvedWorktreeDirectory(slug: String, branch: String) throws -> String {
        guard !project.isRemote else {
            let trimmed = remotePath.trimmingCharacters(in: .whitespaces)
            return trimmed.isEmpty
                ? WorktreeLocationResolver.remoteWorktreeDirectory(for: project, slug: slug)
                : trimmed
        }
        return try resolvedLocalWorktreeDirectory(slug: slug, branch: branch)
    }

    private func resolvedLocalWorktreeDirectory(slug: String, branch: String) throws -> String {
        try WorktreeLocationResolver.worktreeDirectory(
            for: resolvedProject,
            slug: slug,
            branch: branch,
            defaultPathTemplate: defaultWorktreePathTemplate,
            defaultParentPath: defaultWorktreeParentPath
        )
    }

    private func validateLocalLocationSelection() throws {
        switch localLocationSelection.mode {
        case .defaultLocation:
            return
        case .pathTemplate:
            _ = try WorktreeLocationResolver.validatedPathTemplate(localLocationSelection.value)
        case .parentFolder:
            guard WorktreeLocationResolver.normalizedLocation(localLocationSelection.value) != nil else {
                throw WorktreeLocationError.parentFolderRequired
            }
        }
    }
}

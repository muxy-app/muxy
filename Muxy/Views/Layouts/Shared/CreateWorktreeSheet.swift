import AppKit
import SwiftUI

enum CreateWorktreeResult {
    case created(Worktree, runSetup: Bool)
    case cancelled
}

struct CreateWorktreeSheet: View {
    let project: Project
    let onFinish: (CreateWorktreeResult) -> Void

    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectStore.self) private var projectStore
    @AppStorage(GeneralSettingsKeys.defaultWorktreePathTemplate)
    private var defaultWorktreePathTemplate = ""
    @AppStorage(GeneralSettingsKeys.defaultWorktreeParentPath)
    private var defaultWorktreeParentPath = ""
    @State private var name: String = ""
    @State private var branchName: String = ""
    @State private var branchNameEdited = false
    @State private var createNewBranch = true
    @State private var selectedExistingBranch: String = ""
    @State private var localLocationSelection = WorktreeLocationSelection()
    @State private var availableBranches: [String] = []
    @State private var selectedBaseBranch: String = ""
    @State private var setupCommands: [String] = []
    @State private var runSetup = false
    @State private var inProgress = false
    @State private var errorMessage: String?
    @State private var remotePath: String = ""
    @State private var remotePathEdited = false

    private var workspaceContext: WorkspaceContext {
        ActiveWorkspaceContext.shared.current
    }

    private var gitRepository: GitRepositoryService {
        GitRepositoryService(context: workspaceContext)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text("New Worktree")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                Text("Name").font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                TextField("feature-x", text: $name)
                    .textFieldStyle(.roundedBorder)
            }

            SegmentedPicker(
                selection: $createNewBranch,
                options: [(true, "Create new branch"), (false, "Use existing branch")]
            )

            if createNewBranch {
                VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                    Text("Branch Name").font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                    TextField("feature-x", text: $branchName)
                        .textFieldStyle(.roundedBorder)
                        .onChange(of: branchName) { _, newValue in
                            branchNameEdited = newValue != name
                        }
                }
                VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                    Text("Base Branch").font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                    Picker("", selection: $selectedBaseBranch) {
                        ForEach(availableBranches, id: \.self) { branch in
                            Text(branch).tag(branch)
                        }
                    }
                    .labelsHidden()
                    .disabled(availableBranches.isEmpty)
                }
            } else {
                VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                    Text("Branch").font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                    Picker("", selection: $selectedExistingBranch) {
                        ForEach(availableBranches, id: \.self) { branch in
                            Text(branch).tag(branch)
                        }
                    }
                    .labelsHidden()
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
                Button("Cancel") { onFinish(.cancelled) }
                    .keyboardShortcut(.cancelAction)
                Button("Create") { Task { await create() } }
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
            Text("Location").font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
            if project.isRemote {
                remoteLocationField
            } else {
                localLocationRow
            }
        }
    }

    private var remoteLocationField: some View {
        TextField("~/.muxy-worktrees/<name>", text: $remotePath)
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
                    (.defaultLocation, "Default"),
                    (.pathTemplate, "Template"),
                    (.parentFolder, "Folder"),
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
                    TextField("/path/to/worktrees", text: localLocationText)
                        .textFieldStyle(.roundedBorder)
                        .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))

                    Button("Choose Folder...") {
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

            Text("Templates must include {branch}. Relative paths start from the project folder.")
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
                Text("Setup commands from .muxy/worktree.json")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
            }
            Text("These commands will run in the new worktree's terminal. Only enable this if you trust this repository.")
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
            Toggle("Run these commands after creating the worktree", isOn: $runSetup)
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
                Text("Optional setup commands")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
            }
            Text("To run setup commands after creating a worktree, add .muxy/worktree.json in this repository.")
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
            Text("\(project.path)/.muxy/worktree.json")
                .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                .foregroundStyle(MuxyTheme.fg)
                .textSelection(.enabled)
            Text("{\n  \"setup\": [\n    \"pnpm install\",\n    \"pnpm dev\"\n  ]\n}")
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
        let branch = createNewBranch ? branchName : selectedExistingBranch
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
        panel.message = "Select where new worktrees for this project should be created"
        let initialPath = worktreeDirectoryPath.isEmpty ? project.path : worktreeDirectoryPath
        panel.directoryURL = URL(fileURLWithPath: initialPath, isDirectory: true).deletingLastPathComponent()
        guard panel.runModal() == .OK, let url = panel.url else { return }
        localLocationSelection.select(.parentFolder)
        localLocationSelection.value = url.path
    }

    private var canCreate: Bool {
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
        return !selectedExistingBranch.isEmpty
    }

    private func loadBranches() async {
        do {
            async let branchesValue = gitRepository.listBranches(repoPath: project.path)
            async let defaultValue = gitRepository.defaultBranch(repoPath: project.path)
            let branches = try await branchesValue
            let resolvedDefault = await defaultValue
            await MainActor.run {
                availableBranches = branches
                if selectedExistingBranch.isEmpty {
                    selectedExistingBranch = branches.first ?? ""
                }
                if selectedBaseBranch.isEmpty {
                    if let resolvedDefault, branches.contains(resolvedDefault) {
                        selectedBaseBranch = resolvedDefault
                    } else {
                        selectedBaseBranch = branches.first ?? ""
                    }
                }
            }
        } catch {
            await MainActor.run {
                errorMessage = error.localizedDescription
            }
        }
    }

    @MainActor
    private func create() async {
        inProgress = true
        errorMessage = nil
        let trimmedName = name.trimmingCharacters(in: .whitespaces)
        let branch = createNewBranch
            ? branchName.trimmingCharacters(in: .whitespaces)
            : selectedExistingBranch

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

        let trimmedBase = selectedBaseBranch.trimmingCharacters(in: .whitespaces)
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

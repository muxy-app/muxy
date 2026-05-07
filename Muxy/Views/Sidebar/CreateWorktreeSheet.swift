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
    @AppStorage(GeneralSettingsKeys.defaultWorktreeParentPath)
    private var defaultWorktreeParentPath = ""
    @State private var name: String = ""
    @State private var branchName: String = ""
    @State private var branchNameEdited = false
    @State private var createNewBranch = true
    @State private var selectedExistingBranch: String = ""
    @State private var selectedParentPath: String?
    @State private var usesProjectLocation = false
    @State private var availableBranches: [String] = []
    @State private var setupCommands: [String] = []
    @State private var runSetup = false
    @State private var inProgress = false
    @State private var errorMessage: String?

    private let gitRepository = GitRepositoryService()
    private let gitWorktree = GitWorktreeService.shared

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
            HStack(spacing: UIMetrics.spacing4) {
                Text(parentDirectoryPath)
                    .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, UIMetrics.spacing4)
                    .padding(.vertical, UIMetrics.spacing3)
                    .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))

                Button("Choose Folder...") {
                    chooseParentDirectory()
                }
                .fixedSize(horizontal: true, vertical: false)

                Button("Use Default") {
                    selectedParentPath = nil
                    usesProjectLocation = false
                }
                .fixedSize(horizontal: true, vertical: false)
                .disabled(!usesProjectLocation)
            }
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
        guard let config = WorktreeConfig.load(fromProjectPath: project.path) else {
            setupCommands = []
            return
        }
        setupCommands = config.setup.map(\.command).filter { !$0.isEmpty }
    }

    private func loadLocation() {
        guard selectedParentPath == nil, !usesProjectLocation else { return }
        guard let path = WorktreeLocationResolver.normalizedPath(project.preferredWorktreeParentPath) else { return }
        selectedParentPath = path
        usesProjectLocation = true
    }

    private var resolvedProject: Project {
        var resolved = project
        resolved.preferredWorktreeParentPath = usesProjectLocation ? selectedParentPath : nil
        return resolved
    }

    private var parentDirectoryPath: String {
        WorktreeLocationResolver
            .parentDirectory(for: resolvedProject, defaultParentPath: defaultWorktreeParentPath)
            .path
    }

    private func chooseParentDirectory() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.message = "Select where new worktrees for this project should be created"
        panel.directoryURL = URL(fileURLWithPath: parentDirectoryPath, isDirectory: true)
        guard panel.runModal() == .OK, let url = panel.url else { return }
        selectedParentPath = url.path
        usesProjectLocation = true
    }

    private var canCreate: Bool {
        guard !name.trimmingCharacters(in: .whitespaces).isEmpty else { return false }
        if createNewBranch {
            return !branchName.trimmingCharacters(in: .whitespaces).isEmpty
        }
        return !selectedExistingBranch.isEmpty
    }

    private func loadBranches() async {
        do {
            let branches = try await gitRepository.listBranches(repoPath: project.path)
            await MainActor.run {
                availableBranches = branches
                if selectedExistingBranch.isEmpty {
                    selectedExistingBranch = branches.first ?? ""
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

        let slug = Self.slug(from: trimmedName)
        let parentDirectory = parentDirectoryPath
        let worktreeDirectory = URL(fileURLWithPath: parentDirectory, isDirectory: true)
            .appendingPathComponent(slug, isDirectory: true)
            .path

        if FileManager.default.fileExists(atPath: worktreeDirectory) {
            inProgress = false
            errorMessage = "A worktree with this name already exists on disk."
            return
        }

        do {
            try await GitProcessRunner.offMainThrowing {
                try FileManager.default.createDirectory(
                    atPath: parentDirectory,
                    withIntermediateDirectories: true,
                    attributes: nil
                )
            }
        } catch {
            inProgress = false
            errorMessage = error.localizedDescription
            return
        }

        do {
            try await gitWorktree.addWorktree(
                repoPath: project.path,
                path: worktreeDirectory,
                branch: branch,
                createBranch: createNewBranch
            )
        } catch {
            inProgress = false
            errorMessage = error.localizedDescription
            return
        }

        let worktree = Worktree(
            name: trimmedName,
            path: worktreeDirectory,
            branch: branch,
            ownsBranch: createNewBranch,
            isPrimary: false
        )
        projectStore.setPreferredWorktreeParentPath(
            id: project.id,
            to: usesProjectLocation ? selectedParentPath : nil
        )
        worktreeStore.add(worktree, to: project.id)
        inProgress = false
        onFinish(.created(worktree, runSetup: runSetup))
    }

    private static func slug(from name: String) -> String {
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "._-"))
        let scalars = name.unicodeScalars.map { allowed.contains($0) ? Character($0) : "-" }
        let collapsed = String(scalars)
            .split(separator: "-", omittingEmptySubsequences: true)
            .joined(separator: "-")
        return collapsed.isEmpty ? UUID().uuidString : collapsed
    }
}

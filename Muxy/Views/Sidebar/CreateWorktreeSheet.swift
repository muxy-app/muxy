import SwiftUI

enum CreateWorktreeResult {
    case created(Worktree, runSetup: Bool)
    case cancelled
}

struct CreateWorktreeSheet: View {
    let project: Project
    let onFinish: (CreateWorktreeResult) -> Void

    @Environment(WorktreeStore.self) private var worktreeStore
    @State private var name: String = ""
    @State private var branchName: String = ""
    @State private var branchNameEdited = false
    @State private var createNewBranch = true
    @State private var selectedExistingBranch: String = ""
    @State private var availableBranches: [String] = []
    @State private var setupCommands: [String] = []
    @State private var runSetup = false
    @State private var inProgress = false
    @State private var errorMessage: String?

    private let gitRepository = GitRepositoryService()
    private let gitWorktree = GitWorktreeService.shared

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("New Worktree")
                .font(.system(size: 14, weight: .semibold))

            VStack(alignment: .leading, spacing: 6) {
                Text("Name").font(.system(size: 11)).foregroundStyle(MuxyTheme.fgMuted)
                TextField("feature-x", text: $name)
                    .textFieldStyle(.roundedBorder)
            }

            Picker("", selection: $createNewBranch) {
                Text("Create new branch").tag(true)
                Text("Use existing branch").tag(false)
            }
            .pickerStyle(.segmented)
            .labelsHidden()

            if createNewBranch {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Branch Name").font(.system(size: 11)).foregroundStyle(MuxyTheme.fgMuted)
                    TextField("feature-x", text: $branchName)
                        .textFieldStyle(.roundedBorder)
                        .onChange(of: branchName) { _, newValue in
                            branchNameEdited = newValue != name
                        }
                }
            } else {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Branch").font(.system(size: 11)).foregroundStyle(MuxyTheme.fgMuted)
                    Picker("", selection: $selectedExistingBranch) {
                        ForEach(availableBranches, id: \.self) { branch in
                            Text(branch).tag(branch)
                        }
                    }
                    .labelsHidden()
                }
            }

            if setupCommands.isEmpty {
                setupCommandsGuideSection
            } else {
                setupCommandsSection
            }

            if let errorMessage {
                Text(errorMessage)
                    .font(.system(size: 11))
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
        .padding(20)
        .frame(width: 460)
        .task {
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

    private var setupCommandsSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 6) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 10))
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
                Text("Setup commands from .muxy/worktree.json")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
            }
            Text("These commands will run in the new worktree's terminal. Only enable this if you trust this repository.")
                .font(.system(size: 10))
                .foregroundStyle(MuxyTheme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
            VStack(alignment: .leading, spacing: 2) {
                ForEach(setupCommands, id: \.self) { command in
                    Text(command)
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fg)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .padding(8)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 4))
            Toggle("Run these commands after creating the worktree", isOn: $runSetup)
                .font(.system(size: 11))
        }
        .padding(10)
        .background(MuxyTheme.hover, in: RoundedRectangle(cornerRadius: 6))
    }

    private var setupCommandsGuideSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 6) {
                Image(systemName: "info.circle")
                    .font(.system(size: 10))
                    .foregroundStyle(MuxyTheme.fgDim)
                Text("Optional setup commands")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
            }
            Text("To run setup commands after creating a worktree, add .muxy/worktree.json in this repository.")
                .font(.system(size: 10))
                .foregroundStyle(MuxyTheme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
            Text("\(project.path)/.muxy/worktree.json")
                .font(.system(size: 10, design: .monospaced))
                .foregroundStyle(MuxyTheme.fg)
                .textSelection(.enabled)
            Text("{\n  \"setup\": [\n    \"pnpm install\",\n    \"pnpm dev\"\n  ]\n}")
                .font(.system(size: 10, design: .monospaced))
                .foregroundStyle(MuxyTheme.fg)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(8)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 4))
        }
        .padding(10)
        .background(MuxyTheme.hover, in: RoundedRectangle(cornerRadius: 6))
    }

    private func loadSetupCommands() {
        guard let config = WorktreeConfig.load(fromProjectPath: project.path) else {
            setupCommands = []
            return
        }
        setupCommands = config.setup.map(\.command).filter { !$0.isEmpty }
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

    private func create() async {
        inProgress = true
        errorMessage = nil
        let trimmedName = name.trimmingCharacters(in: .whitespaces)
        let branch = createNewBranch
            ? branchName.trimmingCharacters(in: .whitespaces)
            : selectedExistingBranch

        let slug = Self.slug(from: trimmedName)
        let worktreeDirectory = MuxyFileStorage
            .worktreeDirectory(forProjectID: project.id, name: slug)
            .path(percentEncoded: false)

        if FileManager.default.fileExists(atPath: worktreeDirectory) {
            await MainActor.run {
                inProgress = false
                errorMessage = "A worktree with this name already exists on disk."
            }
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
            await MainActor.run {
                inProgress = false
                errorMessage = error.localizedDescription
            }
            return
        }

        let worktree = Worktree(
            name: trimmedName,
            path: worktreeDirectory,
            branch: branch,
            ownsBranch: createNewBranch,
            isPrimary: false
        )
        await MainActor.run {
            worktreeStore.add(worktree, to: project.id)
            inProgress = false
            onFinish(.created(worktree, runSetup: runSetup))
        }
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

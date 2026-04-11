import SwiftUI

enum CreateWorktreeResult {
    case created(Worktree, runSetup: Bool)
    case cancelled
    case failed(String)
}

struct CreateWorktreeSheet: View {
    let project: Project
    let onFinish: (CreateWorktreeResult) -> Void

    @Environment(WorktreeStore.self) private var worktreeStore
    @State private var name: String = ""
    @State private var branchName: String = ""
    @State private var createNewBranch = true
    @State private var selectedExistingBranch: String = ""
    @State private var availableBranches: [String] = []
    @State private var runSetup = true
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

            Toggle("Run setup commands from .muxy/worktree.json", isOn: $runSetup)
                .font(.system(size: 11))

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
        .frame(width: 420)
        .task { await loadBranches() }
        .onChange(of: name) { _, newValue in
            if createNewBranch, branchName.isEmpty || branchName == oldNameToBranch {
                branchName = newValue
            }
            oldNameToBranch = branchName
        }
    }

    @State private var oldNameToBranch: String = ""

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

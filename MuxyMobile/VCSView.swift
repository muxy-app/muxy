import MuxyShared
import SwiftUI

struct VCSView: View {
    let projectID: UUID
    @Environment(ConnectionManager.self) private var connection
    @Environment(\.dismiss) private var dismiss

    @State private var status: VCSStatusDTO?
    @State private var isLoading = false
    @State private var errorMessage: String?
    @State private var commitMessage = ""
    @State private var inFlight: Set<String> = []
    @State private var showingBranches = false
    @State private var showingWorktrees = false
    @State private var showingCreatePR = false

    var body: some View {
        NavigationStack {
            ZStack {
                themeBg.ignoresSafeArea()
                content
            }
            .navigationTitle("Source Control")
            .navigationBarTitleDisplayMode(.inline)
            .toolbarColorScheme(preferredScheme, for: .navigationBar)
            .tint(themeFg)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Done") { dismiss() }
                        .foregroundStyle(themeFg)
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Button {
                            showingBranches = true
                        } label: {
                            Label("Branches", systemImage: "arrow.triangle.branch")
                        }
                        Button {
                            showingWorktrees = true
                        } label: {
                            Label("Worktrees", systemImage: "square.stack.3d.up")
                        }
                        if status?.pullRequest == nil {
                            Button {
                                showingCreatePR = true
                            } label: {
                                Label("Create Pull Request", systemImage: "arrow.up.square")
                            }
                        }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                            .foregroundStyle(themeFg)
                    }
                }
            }
            .sheet(isPresented: $showingBranches) {
                BranchesSheet(projectID: projectID) { await refresh() }
            }
            .sheet(isPresented: $showingWorktrees) {
                WorktreesSheet(projectID: projectID) { await refresh() }
            }
            .sheet(isPresented: $showingCreatePR) {
                CreatePRSheet(
                    projectID: projectID,
                    defaultBase: status?.defaultBranch,
                    currentBranch: status?.branch ?? ""
                ) { await refresh() }
            }
        }
        .task { await refresh() }
    }

    @ViewBuilder
    private var content: some View {
        if let status {
            List {
                summarySection(status)
                if !status.stagedFiles.isEmpty {
                    stagedSection(status.stagedFiles)
                }
                if !status.changedFiles.isEmpty {
                    changesSection(status.changedFiles)
                }
                if status.stagedFiles.isEmpty, status.changedFiles.isEmpty {
                    cleanSection
                }
                if !status.stagedFiles.isEmpty {
                    commitSection
                }
                if let error = errorMessage {
                    Section {
                        Text(error)
                            .font(.footnote)
                            .foregroundStyle(.red)
                    }
                    .listRowBackground(themeFg.opacity(0.06))
                }
            }
            .scrollContentBackground(.hidden)
            .refreshable { await refresh() }
        } else if isLoading {
            ProgressView().tint(themeFg)
        } else {
            VStack(spacing: 12) {
                Image(systemName: "arrow.triangle.branch")
                    .font(.system(size: 40))
                    .foregroundStyle(themeFg.opacity(0.4))
                Text("Could not load repository status")
                    .foregroundStyle(themeFg.opacity(0.7))
                if let error = errorMessage {
                    Text(error)
                        .font(.footnote)
                        .foregroundStyle(.red)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal)
                }
                Button("Retry") { Task { await refresh() } }
                    .buttonStyle(.borderedProminent)
                    .tint(themeFg)
            }
        }
    }

    private func summarySection(_ status: VCSStatusDTO) -> some View {
        Section {
            HStack {
                Image(systemName: "arrow.triangle.branch")
                    .foregroundStyle(themeFg.opacity(0.7))
                Text(status.branch)
                    .font(.body.weight(.medium))
                    .foregroundStyle(themeFg)
                Spacer()
                if status.aheadCount > 0 {
                    Label("\(status.aheadCount)", systemImage: "arrow.up")
                        .labelStyle(.titleAndIcon)
                        .font(.caption)
                        .foregroundStyle(themeFg.opacity(0.7))
                }
                if status.behindCount > 0 {
                    Label("\(status.behindCount)", systemImage: "arrow.down")
                        .labelStyle(.titleAndIcon)
                        .font(.caption)
                        .foregroundStyle(themeFg.opacity(0.7))
                }
            }

            if let pr = status.pullRequest {
                HStack {
                    Image(systemName: "arrow.up.square")
                        .foregroundStyle(themeFg.opacity(0.7))
                    Link("PR #\(pr.number) (\(pr.state.lowercased()))", destination: URL(string: pr.url)!)
                        .foregroundStyle(themeFg)
                        .font(.footnote)
                    Spacer()
                }
            }

            HStack(spacing: 12) {
                Button {
                    Task { await run("pull") { try await connection.vcsPull(projectID: projectID) } }
                } label: {
                    Label("Pull", systemImage: "arrow.down")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .tint(themeFg)
                .disabled(inFlight.contains("pull"))

                Button {
                    Task { await run("push") { try await connection.vcsPush(projectID: projectID) } }
                } label: {
                    Label("Push", systemImage: "arrow.up")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .tint(themeFg)
                .disabled(inFlight.contains("push") || status.aheadCount == 0 && status.hasUpstream)
            }
        }
        .listRowBackground(themeFg.opacity(0.06))
    }

    private func stagedSection(_ files: [GitFileDTO]) -> some View {
        Section {
            ForEach(files) { file in
                fileRow(file, staged: true)
            }
        } header: {
            HStack {
                Text("Staged (\(files.count))")
                Spacer()
                Button("Unstage All") {
                    Task {
                        await run("unstageAll") {
                            try await connection.unstageFiles(
                                projectID: projectID,
                                paths: files.map(\.path)
                            )
                        }
                    }
                }
                .font(.caption)
                .foregroundStyle(themeFg)
            }
            .foregroundStyle(themeFg.opacity(0.7))
        }
        .listRowBackground(themeFg.opacity(0.06))
    }

    private func changesSection(_ files: [GitFileDTO]) -> some View {
        Section {
            ForEach(files) { file in
                fileRow(file, staged: false)
            }
        } header: {
            HStack {
                Text("Changes (\(files.count))")
                Spacer()
                Button("Stage All") {
                    Task {
                        await run("stageAll") {
                            try await connection.stageFiles(
                                projectID: projectID,
                                paths: files.map(\.path)
                            )
                        }
                    }
                }
                .font(.caption)
                .foregroundStyle(themeFg)
            }
            .foregroundStyle(themeFg.opacity(0.7))
        }
        .listRowBackground(themeFg.opacity(0.06))
    }

    private var cleanSection: some View {
        Section {
            HStack {
                Image(systemName: "checkmark.circle")
                    .foregroundStyle(.green)
                Text("Working tree clean")
                    .foregroundStyle(themeFg.opacity(0.7))
            }
        }
        .listRowBackground(themeFg.opacity(0.06))
    }

    private var commitSection: some View {
        Section {
            TextField("Commit message", text: $commitMessage, axis: .vertical)
                .lineLimit(2 ... 5)
                .foregroundStyle(themeFg)
            Button {
                Task {
                    await run("commit") {
                        try await connection.vcsCommit(
                            projectID: projectID,
                            message: commitMessage,
                            stageAll: false
                        )
                        commitMessage = ""
                    }
                }
            } label: {
                if inFlight.contains("commit") {
                    ProgressView().tint(themeFg)
                } else {
                    Label("Commit", systemImage: "checkmark")
                        .frame(maxWidth: .infinity)
                }
            }
            .buttonStyle(.borderedProminent)
            .tint(themeFg)
            .disabled(
                commitMessage.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    || inFlight.contains("commit")
            )
        } header: {
            Text("Commit").foregroundStyle(themeFg.opacity(0.7))
        }
        .listRowBackground(themeFg.opacity(0.06))
    }

    private func fileRow(_ file: GitFileDTO, staged: Bool) -> some View {
        HStack(spacing: 10) {
            StatusBadge(status: file.status)
            VStack(alignment: .leading, spacing: 2) {
                Text(fileName(from: file.path))
                    .font(.body)
                    .foregroundStyle(themeFg)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Text(file.path)
                    .font(.caption2)
                    .foregroundStyle(themeFg.opacity(0.5))
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            Spacer()
        }
        .swipeActions(edge: .trailing, allowsFullSwipe: true) {
            if staged {
                Button {
                    Task {
                        await run("unstage:\(file.path)") {
                            try await connection.unstageFiles(projectID: projectID, paths: [file.path])
                        }
                    }
                } label: {
                    Label("Unstage", systemImage: "minus.circle")
                }
                .tint(.orange)
            } else {
                Button {
                    Task {
                        await run("stage:\(file.path)") {
                            try await connection.stageFiles(projectID: projectID, paths: [file.path])
                        }
                    }
                } label: {
                    Label("Stage", systemImage: "plus.circle")
                }
                .tint(.green)

                Button(role: .destructive) {
                    Task {
                        await run("discard:\(file.path)") {
                            if file.isUntracked {
                                try await connection.discardFiles(
                                    projectID: projectID,
                                    paths: [],
                                    untrackedPaths: [file.path]
                                )
                            } else {
                                try await connection.discardFiles(
                                    projectID: projectID,
                                    paths: [file.path],
                                    untrackedPaths: []
                                )
                            }
                        }
                    }
                } label: {
                    Label("Discard", systemImage: "trash")
                }
            }
        }
        .listRowBackground(themeFg.opacity(0.06))
    }

    private func fileName(from path: String) -> String {
        path.components(separatedBy: "/").last ?? path
    }

    private func refresh() async {
        isLoading = true
        errorMessage = nil
        let result = await connection.fetchVCSStatus(projectID: projectID)
        status = result
        isLoading = false
        if result == nil {
            errorMessage = "This project may not be a Git repository."
        }
    }

    private func run(_ key: String, _ op: @escaping () async throws -> Void) async {
        inFlight.insert(key)
        defer { inFlight.remove(key) }
        do {
            try await op()
            errorMessage = nil
            await refresh()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private var themeFg: Color {
        connection.deviceTheme?.fgColor ?? .primary
    }

    private var themeBg: Color {
        connection.deviceTheme?.bgColor ?? Color(.systemBackground)
    }

    private var preferredScheme: ColorScheme {
        (connection.deviceTheme?.isDark ?? true) ? .dark : .light
    }
}

private struct StatusBadge: View {
    let status: GitFileStatusDTO

    var body: some View {
        Text(label)
            .font(.caption2.weight(.bold))
            .frame(width: 20, height: 20)
            .background(color.opacity(0.2))
            .foregroundStyle(color)
            .clipShape(RoundedRectangle(cornerRadius: 4))
    }

    private var label: String {
        switch status {
        case .added: "A"
        case .modified: "M"
        case .deleted: "D"
        case .renamed: "R"
        case .copied: "C"
        case .untracked: "U"
        case .unmerged: "!"
        }
    }

    private var color: Color {
        switch status {
        case .added,
             .untracked: .green
        case .modified,
             .renamed,
             .copied: .orange
        case .deleted: .red
        case .unmerged: .purple
        }
    }
}

struct BranchesSheet: View {
    let projectID: UUID
    let onChange: () async -> Void
    @Environment(ConnectionManager.self) private var connection
    @Environment(\.dismiss) private var dismiss

    @State private var branches: VCSBranchesDTO?
    @State private var isLoading = false
    @State private var errorMessage: String?
    @State private var busyBranch: String?
    @State private var showingCreate = false
    @State private var newBranchName = ""

    var body: some View {
        NavigationStack {
            ZStack {
                themeBg.ignoresSafeArea()
                content
            }
            .navigationTitle("Branches")
            .navigationBarTitleDisplayMode(.inline)
            .toolbarColorScheme(preferredScheme, for: .navigationBar)
            .tint(themeFg)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Close") { dismiss() }.foregroundStyle(themeFg)
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button { showingCreate = true } label: {
                        Image(systemName: "plus").foregroundStyle(themeFg)
                    }
                }
            }
            .alert("New Branch", isPresented: $showingCreate) {
                TextField("branch-name", text: $newBranchName)
                Button("Cancel", role: .cancel) { newBranchName = "" }
                Button("Create") {
                    let name = newBranchName
                    newBranchName = ""
                    Task { await createBranch(name: name) }
                }
            } message: {
                Text("Creates and switches to a new branch from HEAD.")
            }
        }
        .task { await load() }
    }

    @ViewBuilder
    private var content: some View {
        if let branches {
            List {
                ForEach(branches.locals, id: \.self) { branch in
                    branchRow(branch, current: branches.current)
                }
                if let error = errorMessage {
                    Text(error).font(.footnote).foregroundStyle(.red)
                        .listRowBackground(themeFg.opacity(0.06))
                }
            }
            .scrollContentBackground(.hidden)
        } else if isLoading {
            ProgressView().tint(themeFg)
        } else {
            Text(errorMessage ?? "No branches").foregroundStyle(themeFg.opacity(0.7))
        }
    }

    private func branchRow(_ branch: String, current: String) -> some View {
        Button {
            guard branch != current else { return }
            Task { await switchTo(branch) }
        } label: {
            HStack {
                Image(systemName: branch == current ? "checkmark.circle.fill" : "circle")
                    .foregroundStyle(branch == current ? .green : themeFg.opacity(0.4))
                Text(branch)
                    .foregroundStyle(themeFg)
                Spacer()
                if busyBranch == branch {
                    ProgressView().tint(themeFg)
                }
            }
        }
        .listRowBackground(themeFg.opacity(0.06))
    }

    private func load() async {
        isLoading = true
        errorMessage = nil
        do {
            branches = try await connection.listBranches(projectID: projectID)
        } catch {
            errorMessage = error.localizedDescription
        }
        isLoading = false
    }

    private func switchTo(_ branch: String) async {
        busyBranch = branch
        defer { busyBranch = nil }
        do {
            try await connection.switchBranch(projectID: projectID, branch: branch)
            await onChange()
            dismiss()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func createBranch(name: String) async {
        let trimmed = name.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return }
        busyBranch = trimmed
        defer { busyBranch = nil }
        do {
            try await connection.createBranch(projectID: projectID, name: trimmed)
            await onChange()
            dismiss()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private var themeFg: Color { connection.deviceTheme?.fgColor ?? .primary }
    private var themeBg: Color { connection.deviceTheme?.bgColor ?? Color(.systemBackground) }
    private var preferredScheme: ColorScheme { (connection.deviceTheme?.isDark ?? true) ? .dark : .light }
}

struct WorktreesSheet: View {
    let projectID: UUID
    let onChange: () async -> Void
    @Environment(ConnectionManager.self) private var connection
    @Environment(\.dismiss) private var dismiss

    @State private var errorMessage: String?
    @State private var busyID: UUID?
    @State private var showingAdd = false

    private var activeWorktreeID: UUID? {
        connection.workspace?.worktreeID
    }

    var body: some View {
        NavigationStack {
            ZStack {
                themeBg.ignoresSafeArea()
                content
            }
            .navigationTitle("Worktrees")
            .navigationBarTitleDisplayMode(.inline)
            .toolbarColorScheme(preferredScheme, for: .navigationBar)
            .tint(themeFg)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Close") { dismiss() }.foregroundStyle(themeFg)
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button { showingAdd = true } label: {
                        Image(systemName: "plus").foregroundStyle(themeFg)
                    }
                }
            }
            .sheet(isPresented: $showingAdd) {
                AddWorktreeSheet(projectID: projectID)
            }
        }
    }

    @ViewBuilder
    private var content: some View {
        let worktrees = connection.projectWorktrees[projectID] ?? []
        List {
            ForEach(worktrees) { worktree in
                row(worktree)
            }
            if let error = errorMessage {
                Text(error).font(.footnote).foregroundStyle(.red)
                    .listRowBackground(themeFg.opacity(0.06))
            }
        }
        .scrollContentBackground(.hidden)
    }

    private func row(_ worktree: WorktreeDTO) -> some View {
        let isActive = worktree.id == activeWorktreeID
        return Button {
            guard !isActive else { return }
            Task { await switchTo(worktree) }
        } label: {
            HStack {
                Image(systemName: isActive ? "checkmark.circle.fill" : (worktree.isPrimary ? "house" : "square.stack.3d.up"))
                    .foregroundStyle(isActive ? .green : themeFg.opacity(0.7))
                VStack(alignment: .leading, spacing: 2) {
                    Text(worktree.name)
                        .foregroundStyle(themeFg)
                    if let branch = worktree.branch {
                        Text(branch)
                            .font(.caption)
                            .foregroundStyle(themeFg.opacity(0.6))
                    }
                }
                Spacer()
                if busyID == worktree.id {
                    ProgressView().tint(themeFg)
                }
            }
        }
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
            if !worktree.isPrimary, !isActive {
                Button(role: .destructive) {
                    Task { await remove(worktree) }
                } label: {
                    Label("Remove", systemImage: "trash")
                }
            }
        }
        .listRowBackground(themeFg.opacity(0.06))
    }

    private func switchTo(_ worktree: WorktreeDTO) async {
        busyID = worktree.id
        defer { busyID = nil }
        do {
            try await connection.selectWorktree(projectID: projectID, worktreeID: worktree.id)
            await onChange()
            dismiss()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func remove(_ worktree: WorktreeDTO) async {
        busyID = worktree.id
        defer { busyID = nil }
        do {
            try await connection.removeWorktree(projectID: projectID, worktreeID: worktree.id)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private var themeFg: Color { connection.deviceTheme?.fgColor ?? .primary }
    private var themeBg: Color { connection.deviceTheme?.bgColor ?? Color(.systemBackground) }
    private var preferredScheme: ColorScheme { (connection.deviceTheme?.isDark ?? true) ? .dark : .light }
}

struct AddWorktreeSheet: View {
    let projectID: UUID
    @Environment(ConnectionManager.self) private var connection
    @Environment(\.dismiss) private var dismiss

    @State private var name = ""
    @State private var branchName = ""
    @State private var useExistingBranch = false
    @State private var existingBranches: [String] = []
    @State private var selectedExisting = ""
    @State private var inProgress = false
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            ZStack {
                themeBg.ignoresSafeArea()
                Form {
                    Section("Worktree") {
                        TextField("Name", text: $name)
                            .foregroundStyle(themeFg)
                    }
                    .listRowBackground(themeFg.opacity(0.06))

                    Section("Branch") {
                        Picker("Source", selection: $useExistingBranch) {
                            Text("New Branch").tag(false)
                            Text("Existing").tag(true)
                        }
                        .pickerStyle(.segmented)

                        if useExistingBranch {
                            Picker("Branch", selection: $selectedExisting) {
                                ForEach(existingBranches, id: \.self) { Text($0).tag($0) }
                            }
                        } else {
                            TextField("new-branch-name", text: $branchName)
                                .foregroundStyle(themeFg)
                        }
                    }
                    .listRowBackground(themeFg.opacity(0.06))

                    if let error = errorMessage {
                        Text(error).font(.footnote).foregroundStyle(.red)
                            .listRowBackground(themeFg.opacity(0.06))
                    }
                }
                .scrollContentBackground(.hidden)
            }
            .navigationTitle("Add Worktree")
            .navigationBarTitleDisplayMode(.inline)
            .toolbarColorScheme(preferredScheme, for: .navigationBar)
            .tint(themeFg)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Cancel") { dismiss() }.foregroundStyle(themeFg)
                }
                ToolbarItem(placement: .topBarTrailing) {
                    if inProgress {
                        ProgressView().tint(themeFg)
                    } else {
                        Button("Add") { Task { await submit() } }
                            .foregroundStyle(themeFg)
                            .disabled(!canSubmit)
                    }
                }
            }
        }
        .task { await loadBranches() }
    }

    private var canSubmit: Bool {
        guard !name.trimmingCharacters(in: .whitespaces).isEmpty else { return false }
        if useExistingBranch {
            return !selectedExisting.isEmpty
        }
        return !branchName.trimmingCharacters(in: .whitespaces).isEmpty
    }

    private func loadBranches() async {
        do {
            let branches = try await connection.listBranches(projectID: projectID)
            existingBranches = branches.locals
            if selectedExisting.isEmpty { selectedExisting = branches.locals.first ?? "" }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func submit() async {
        inProgress = true
        defer { inProgress = false }
        let branch = useExistingBranch
            ? selectedExisting
            : branchName.trimmingCharacters(in: .whitespaces)
        do {
            try await connection.addWorktree(
                projectID: projectID,
                name: name.trimmingCharacters(in: .whitespaces),
                branch: branch,
                createBranch: !useExistingBranch
            )
            dismiss()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private var themeFg: Color { connection.deviceTheme?.fgColor ?? .primary }
    private var themeBg: Color { connection.deviceTheme?.bgColor ?? Color(.systemBackground) }
    private var preferredScheme: ColorScheme { (connection.deviceTheme?.isDark ?? true) ? .dark : .light }
}

struct CreatePRSheet: View {
    let projectID: UUID
    let defaultBase: String?
    let currentBranch: String
    let onCreated: () async -> Void

    @Environment(ConnectionManager.self) private var connection
    @Environment(\.dismiss) private var dismiss
    @Environment(\.openURL) private var openURL

    @State private var title = ""
    @State private var prBody = ""
    @State private var baseBranch = ""
    @State private var draft = false
    @State private var inProgress = false
    @State private var errorMessage: String?

    var sheetBody: some View {
        NavigationStack {
            ZStack {
                themeBg.ignoresSafeArea()
                Form {
                    Section("Branch") {
                        HStack {
                            Text("From")
                            Spacer()
                            Text(currentBranch).foregroundStyle(themeFg.opacity(0.7))
                        }
                        TextField("Base (e.g. main)", text: $baseBranch)
                            .foregroundStyle(themeFg)
                    }
                    .listRowBackground(themeFg.opacity(0.06))

                    Section("Details") {
                        TextField("Title", text: $title)
                            .foregroundStyle(themeFg)
                        TextField("Body", text: $prBody, axis: .vertical)
                            .lineLimit(4 ... 10)
                            .foregroundStyle(themeFg)
                        Toggle("Draft", isOn: $draft)
                    }
                    .listRowBackground(themeFg.opacity(0.06))

                    if let error = errorMessage {
                        Text(error).font(.footnote).foregroundStyle(.red)
                            .listRowBackground(themeFg.opacity(0.06))
                    }
                }
                .scrollContentBackground(.hidden)
            }
            .navigationTitle("New Pull Request")
            .navigationBarTitleDisplayMode(.inline)
            .toolbarColorScheme(preferredScheme, for: .navigationBar)
            .tint(themeFg)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Cancel") { dismiss() }.foregroundStyle(themeFg)
                }
                ToolbarItem(placement: .topBarTrailing) {
                    if inProgress {
                        ProgressView().tint(themeFg)
                    } else {
                        Button("Create") { Task { await submit() } }
                            .foregroundStyle(themeFg)
                            .disabled(title.trimmingCharacters(in: .whitespaces).isEmpty)
                    }
                }
            }
        }
    }

    var body: some View {
        sheetBody
            .onAppear {
                if baseBranch.isEmpty, let defaultBase {
                    baseBranch = defaultBase
                }
            }
    }

    private func submit() async {
        inProgress = true
        defer { inProgress = false }
        do {
            let result = try await connection.createPullRequest(
                projectID: projectID,
                title: title.trimmingCharacters(in: .whitespacesAndNewlines),
                body: prBody,
                baseBranch: baseBranch.isEmpty ? nil : baseBranch,
                draft: draft
            )
            await onCreated()
            dismiss()
            if let url = URL(string: result.url) {
                openURL(url)
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private var themeFg: Color { connection.deviceTheme?.fgColor ?? .primary }
    private var themeBg: Color { connection.deviceTheme?.bgColor ?? Color(.systemBackground) }
    private var preferredScheme: ColorScheme { (connection.deviceTheme?.isDark ?? true) ? .dark : .light }
}

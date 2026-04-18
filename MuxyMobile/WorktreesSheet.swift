import MuxyShared
import SwiftUI

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
            if worktree.canBeRemoved, !isActive {
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

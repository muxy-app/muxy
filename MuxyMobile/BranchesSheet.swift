import MuxyShared
import SwiftUI

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
        .preferredColorScheme(preferredScheme)
        .presentationBackground(themeBg)
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

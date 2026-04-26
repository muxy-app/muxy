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
                    .environment(connection)
            }
            .sheet(isPresented: $showingWorktrees) {
                WorktreesSheet(projectID: projectID) { await refresh() }
                    .environment(connection)
            }
            .sheet(isPresented: $showingCreatePR) {
                CreatePRSheet(
                    projectID: projectID,
                    defaultBase: status?.defaultBranch,
                    currentBranch: status?.branch ?? ""
                ) { await refresh() }
                    .environment(connection)
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
                Button {
                    Task { await refresh() }
                } label: {
                    Text("Retry").foregroundStyle(themeBg)
                }
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

            if let pr = status.pullRequest, let prURL = URL(string: pr.url) {
                HStack {
                    Image(systemName: "arrow.up.square")
                        .foregroundStyle(themeFg.opacity(0.7))
                    Link("PR #\(pr.number) (\(pr.state.lowercased()))", destination: prURL)
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
                .disabled(inFlight.contains("push") || (status.aheadCount == 0 && status.hasUpstream))
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
                    ProgressView().tint(themeBg)
                } else {
                    Label("Commit", systemImage: "checkmark")
                        .frame(maxWidth: .infinity)
                        .foregroundStyle(themeBg)
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
            errorMessage = "Could not read repository status. This project may not be a Git repository, or the Mac is unreachable."
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

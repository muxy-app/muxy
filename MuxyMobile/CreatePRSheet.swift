import MuxyShared
import SwiftUI

struct CreatePRSheet: View {
    let projectID: UUID
    let currentBranch: String
    let onCreated: () async -> Void

    @Environment(ConnectionManager.self) private var connection
    @Environment(\.dismiss) private var dismiss
    @Environment(\.openURL) private var openURL

    @State private var title = ""
    @State private var prBody = ""
    @State private var baseBranch: String
    @State private var draft = false
    @State private var inProgress = false
    @State private var errorMessage: String?

    init(projectID: UUID, defaultBase: String?, currentBranch: String, onCreated: @escaping () async -> Void) {
        self.projectID = projectID
        self.currentBranch = currentBranch
        self.onCreated = onCreated
        _baseBranch = State(initialValue: defaultBase ?? "")
    }

    var body: some View {
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
        .preferredColorScheme(preferredScheme)
        .presentationBackground(themeBg)
    }

    private func submit() async {
        inProgress = true
        defer { inProgress = false }
        do {
            let result = try await connection.createPullRequest(
                projectID: projectID,
                title: title.trimmingCharacters(in: .whitespacesAndNewlines),
                body: prBody,
                baseBranch: baseBranch.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    ? nil
                    : baseBranch.trimmingCharacters(in: .whitespacesAndNewlines),
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

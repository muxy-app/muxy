import SwiftUI

struct CreateBranchSheet: View {
    let currentBranch: String?
    let onCreate: (String) -> Void
    let onCancel: () -> Void

    @State private var name: String = ""
    @FocusState private var nameFocused: Bool

    private var trimmed: String {
        name.trimmingCharacters(in: .whitespaces)
    }

    private var canCreate: Bool {
        !trimmed.isEmpty
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text("New Branch")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                Text("Branch Name")
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fgMuted)
                TextField("feature-x", text: $name)
                    .textFieldStyle(.roundedBorder)
                    .focused($nameFocused)
                    .onSubmit { if canCreate { onCreate(trimmed) } }
            }

            if let currentBranch {
                Text("Created from \(currentBranch)")
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fgDim)
            }

            HStack {
                Spacer()
                Button("Cancel", action: onCancel)
                    .keyboardShortcut(.cancelAction)
                Button("Create") { onCreate(trimmed) }
                    .keyboardShortcut(.defaultAction)
                    .disabled(!canCreate)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(360))
        .onAppear { nameFocused = true }
    }
}

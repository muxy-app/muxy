import SwiftUI

struct AISettingsView: View {
    @AppStorage(RepositoryAIAction.commit.providerKey) private var commitProviderID = RepositoryAIActionPreferences.automaticProviderID
    @AppStorage(RepositoryAIAction.commit.promptKey) private var commitPrompt = RepositoryAIAction.commit.defaultPrompt
    @AppStorage(RepositoryAIAction.createPullRequest.providerKey) private var pullRequestProviderID = RepositoryAIActionPreferences
        .automaticProviderID
    @AppStorage(RepositoryAIAction.createPullRequest.promptKey) private var pullRequestPrompt = RepositoryAIAction.createPullRequest
        .defaultPrompt

    private var providers: [any AIAgentLaunchProvider] {
        AIProviderRegistry.shared.agentLaunchProviders
    }

    var body: some View {
        SettingsContainer {
            RepositoryAIActionSettingsSection(
                action: .commit,
                providers: providers,
                providerID: $commitProviderID,
                prompt: $commitPrompt
            )
            RepositoryAIActionSettingsSection(
                action: .createPullRequest,
                providers: providers,
                providerID: $pullRequestProviderID,
                prompt: $pullRequestPrompt,
                showsDivider: false
            )
        }
    }
}

private struct RepositoryAIActionSettingsSection: View {
    let action: RepositoryAIAction
    let providers: [any AIAgentLaunchProvider]
    @Binding var providerID: String
    @Binding var prompt: String
    var showsDivider = true

    var body: some View {
        SettingsSection(
            action.settingsTitle,
            footer: footer,
            showsDivider: showsDivider
        ) {
            SettingsRow("Provider") {
                Picker("", selection: $providerID) {
                    Text("Auto").tag(RepositoryAIActionPreferences.automaticProviderID)
                    ForEach(providers, id: \.id) { provider in
                        Text(provider.displayName).tag(provider.id)
                    }
                }
                .labelsHidden()
                .frame(width: SettingsMetrics.controlWidth, alignment: .trailing)
            }
            promptEditor
        }
    }

    private var promptEditor: some View {
        VStack(alignment: .leading, spacing: SettingsMetrics.rowVerticalPadding) {
            HStack {
                Text("Prompt")
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .foregroundStyle(SettingsStyle.foreground)
                Spacer()
                Button("Restore Default") {
                    prompt = action.defaultPrompt
                }
                .buttonStyle(.plain)
                .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium))
                .foregroundStyle(SettingsStyle.accent)
                .disabled(prompt == action.defaultPrompt)
            }
            TextEditor(text: $prompt)
                .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                .scrollContentBackground(.hidden)
                .settingsTextInput(maxWidth: .infinity, minHeight: 88)
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }

    private var footer: String {
        switch action {
        case .commit:
            "AI generates only the commit message. Muxy always stages all changes, commits, and pushes. "
                + "An empty prompt uses the default. Do not include secrets."
        case .createPullRequest:
            "AI generates the title, summary, new branch name, and target branch. "
                + "Muxy creates the branch, commit, push, and pull request. An empty prompt uses the default."
        }
    }
}

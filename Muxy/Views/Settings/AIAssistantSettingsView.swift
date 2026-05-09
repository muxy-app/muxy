import SwiftUI

struct AIAssistantSettingsView: View {
    @AppStorage(AIAssistantSettings.providerKey) private var providerRaw = AIAssistantProvider.claude.rawValue
    @AppStorage(AIAssistantSettings.claudeModelKey) private var claudeModel = ""
    @AppStorage(AIAssistantSettings.codexModelKey) private var codexModel = ""
    @AppStorage(AIAssistantSettings.opencodeModelKey) private var opencodeModel = ""
    @AppStorage(AIAssistantSettings.customCommandKey) private var customCommand = ""
    @AppStorage(AIAssistantSettings.commitPromptKey) private var commitPrompt = ""
    @AppStorage(AIAssistantSettings.prPromptKey) private var prPrompt = ""

    private var provider: AIAssistantProvider {
        AIAssistantProvider(rawValue: providerRaw) ?? .claude
    }

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Provider",
                footer: "Choose the agentic CLI tool used to generate commit messages and pull request drafts. "
                    + "The tool runs locally with your existing authentication."
            ) {
                SettingsRow("Tool") {
                    Picker("", selection: $providerRaw) {
                        ForEach(AIAssistantProvider.allCases) { provider in
                            Text(provider.displayName).tag(provider.rawValue)
                        }
                    }
                    .labelsHidden()
                    .frame(width: SettingsMetrics.controlWidth, alignment: .trailing)
                }

                if provider != .custom {
                    SettingsRow("Model (optional)") {
                        TextField("Default", text: modelBinding)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: SettingsMetrics.controlWidth)
                    }
                } else {
                    customCommandRow
                }
            }

            SettingsSection(
                "Commit Prompt",
                footer: "Guides the model when generating commit messages. Output is plain text."
            ) {
                promptEditor(
                    text: $commitPrompt,
                    onReset: { commitPrompt = AIAssistantPrompts.defaultCommitUserPrompt }
                )
            }

            SettingsSection(
                "Pull Request Prompt",
                footer: "Guides the model when generating PR title and description. "
                    + "Output is parsed as JSON; do not change the response format."
            ) {
                promptEditor(
                    text: $prPrompt,
                    onReset: { prPrompt = AIAssistantPrompts.defaultPullRequestUserPrompt }
                )
            }
        }
        .onAppear(perform: seedDefaultPromptsIfNeeded)
    }

    private func seedDefaultPromptsIfNeeded() {
        if commitPrompt.isEmpty {
            commitPrompt = AIAssistantPrompts.defaultCommitUserPrompt
        }
        if prPrompt.isEmpty {
            prPrompt = AIAssistantPrompts.defaultPullRequestUserPrompt
        }
    }

    private var modelBinding: Binding<String> {
        switch provider {
        case .claude: $claudeModel
        case .codex: $codexModel
        case .opencode: $opencodeModel
        case .custom: .constant("")
        }
    }

    private var customCommandRow: some View {
        VStack(alignment: .leading, spacing: 6) {
            SettingsRow("Command") {
                TextField("e.g. mytool --quiet", text: $customCommand)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: SettingsMetrics.controlWidth)
            }
            Text(
                "Muxy pipes the full prompt and diff to your command via stdin and reads the response from stdout. "
                    + "Provide arguments that make the tool emit only the response (no banners or progress). "
                    + "Use quotes around arguments containing spaces."
            )
            .font(.system(size: SettingsMetrics.footnoteFontSize))
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, SettingsMetrics.horizontalPadding)
            .padding(.bottom, 4)
        }
    }

    private func promptEditor(
        text: Binding<String>,
        onReset: @escaping () -> Void
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            TextEditor(text: text)
                .font(.system(size: SettingsMetrics.footnoteFontSize))
                .scrollContentBackground(.hidden)
                .padding(.horizontal, 4)
                .padding(.vertical, 2)
                .frame(height: 120)
                .background(.quaternary, in: RoundedRectangle(cornerRadius: 6))
                .padding(.horizontal, SettingsMetrics.horizontalPadding)

            HStack {
                Spacer()
                Button("Reset to default", action: onReset)
                    .buttonStyle(.borderless)
                    .controlSize(.small)
            }
            .padding(.horizontal, SettingsMetrics.horizontalPadding)
            .padding(.bottom, 4)
        }
    }
}

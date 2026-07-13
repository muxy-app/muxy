import Foundation
import Testing

@testable import Muxy

@Suite("Repository AI action preferences")
struct RepositoryAIActionPreferencesTests {
    @Test("actions use metadata-only prompt defaults")
    func defaultPreferences() {
        let defaults = makeDefaults()

        for action in RepositoryAIAction.allCases {
            #expect(RepositoryAIActionPreferences.configuredProviderID(for: action, defaults: defaults).isEmpty)
            #expect(RepositoryAIActionPreferences.prompt(for: action, defaults: defaults) == action.defaultPrompt)
        }
        #expect(RepositoryAIAction.commit.defaultPrompt.contains("commit message"))
        #expect(RepositoryAIAction.createPullRequest.defaultPrompt.contains("pull request title"))
        #expect(RepositoryAIAction.commit.defaultPrompt.contains("staged changes"))
        #expect(RepositoryAIAction.createPullRequest.defaultPrompt.contains("branch name"))
    }

    @Test("stored providers and prompts are action-specific")
    func storedPreferences() {
        let defaults = makeDefaults()
        defaults.set("codex", forKey: RepositoryAIAction.commit.providerKey)
        defaults.set("Use Conventional Commits", forKey: RepositoryAIAction.commit.promptKey)
        defaults.set("claude", forKey: RepositoryAIAction.createPullRequest.providerKey)
        defaults.set("Keep the summary to two sentences", forKey: RepositoryAIAction.createPullRequest.promptKey)

        #expect(RepositoryAIActionPreferences.configuredProviderID(for: .commit, defaults: defaults) == "codex")
        #expect(RepositoryAIActionPreferences.prompt(for: .commit, defaults: defaults) == "Use Conventional Commits")
        #expect(RepositoryAIActionPreferences.configuredProviderID(for: .createPullRequest, defaults: defaults) == "claude")
        #expect(RepositoryAIActionPreferences.prompt(for: .createPullRequest, defaults: defaults) == "Keep the summary to two sentences")
    }

    @Test("blank stored prompts fall back to the workflow default")
    func blankPromptUsesDefault() {
        let defaults = makeDefaults()
        defaults.set(" \n ", forKey: RepositoryAIAction.commit.promptKey)

        #expect(RepositoryAIActionPreferences.prompt(for: .commit, defaults: defaults) == RepositoryAIAction.commit.defaultPrompt)
    }

    @Test("commit presentation protects repository state")
    func commitPresentation() {
        #expect(RepositoryAIActionPresentation.commit(
            isDirty: nil,
            isDetached: nil,
            isRepositoryBusy: false,
            hasRunningAction: false
        ) == .disabled("Loading repository status."))
        #expect(RepositoryAIActionPresentation.commit(
            isDirty: true,
            isDetached: false,
            isRepositoryBusy: false,
            hasRunningAction: false
        ) == .available)
        #expect(RepositoryAIActionPresentation.commit(
            isDirty: false,
            isDetached: false,
            isRepositoryBusy: false,
            hasRunningAction: false
        ) == .disabled("The working tree is clean."))
        #expect(RepositoryAIActionPresentation.commit(
            isDirty: true,
            isDetached: true,
            isRepositoryBusy: false,
            hasRunningAction: false
        ) == .disabled("Switch to a branch before committing and pushing."))
        #expect(RepositoryAIActionPresentation.commit(
            isDirty: true,
            isDetached: false,
            isRepositoryBusy: true,
            hasRunningAction: false
        ) == .disabled("Wait for the current repository action to finish."))
        #expect(RepositoryAIActionPresentation.commit(
            isDirty: true,
            isDetached: false,
            isRepositoryBusy: false,
            hasRunningAction: true
        ) == .disabled("Wait for the current AI repository action to finish."))
    }

    @Test("create PR is visible only after confirming no pull request exists")
    func createPullRequestPresentation() {
        for presence in [
            RepositoryPullRequestPresence.loading,
            .unavailable,
            .found,
        ] {
            #expect(RepositoryAIActionPresentation.createPullRequest(
                pullRequest: presence,
                isDetached: false,
                isRepositoryBusy: false,
                hasRunningAction: false
            ) == .hidden)
        }
        #expect(RepositoryAIActionPresentation.createPullRequest(
            pullRequest: .none,
            isDetached: false,
            isRepositoryBusy: false,
            hasRunningAction: false
        ) == .available)
        #expect(RepositoryAIActionPresentation.createPullRequest(
            pullRequest: .none,
            isDetached: true,
            isRepositoryBusy: false,
            hasRunningAction: false
        ) == .disabled("Switch to a branch before creating a pull request."))
    }

    private func makeDefaults() -> UserDefaults {
        let suiteName = "RepositoryAIActionPreferencesTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            fatalError("Unable to create isolated UserDefaults suite")
        }
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}

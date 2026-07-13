import Foundation

enum RepositoryAIAction: String, CaseIterable, Identifiable {
    case commit
    case createPullRequest

    var id: String { rawValue }

    var title: String {
        switch self {
        case .commit: "Commit"
        case .createPullRequest: "Create PR"
        }
    }

    var runningTitle: String {
        switch self {
        case .commit: "Committing"
        case .createPullRequest: "Creating PR"
        }
    }

    var settingsTitle: String {
        switch self {
        case .commit: "Commit and Push"
        case .createPullRequest: "Create Pull Request"
        }
    }

    var symbolName: String {
        switch self {
        case .commit: "arrow.up.circle"
        case .createPullRequest: "arrow.triangle.pull"
        }
    }

    var providerKey: String {
        "muxy.ai.repositoryActions.\(rawValue).provider"
    }

    var promptKey: String {
        "muxy.ai.repositoryActions.\(rawValue).prompt"
    }

    var defaultPrompt: String {
        switch self {
        case .commit:
            "Write a concise commit message that explains the intent of all staged changes. "
                + "Follow the repository's existing commit-message style."
        case .createPullRequest:
            "Write an accurate pull request title and a concise summary of the changes. "
                + "Choose a short descriptive branch name and the appropriate target branch."
        }
    }
}

enum RepositoryAIActionPreferences {
    static let automaticProviderID = ""

    static func configuredProviderID(
        for action: RepositoryAIAction,
        defaults: UserDefaults = .standard
    ) -> String {
        defaults.string(forKey: action.providerKey) ?? automaticProviderID
    }

    static func prompt(
        for action: RepositoryAIAction,
        defaults: UserDefaults = .standard
    ) -> String {
        guard let stored = defaults.string(forKey: action.promptKey),
              !stored.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else { return action.defaultPrompt }
        return stored
    }
}

enum RepositoryAIActionAvailability: Equatable {
    case available
    case disabled(String)
    case hidden
}

enum RepositoryPullRequestPresence: Equatable {
    case loading
    case none
    case unavailable
    case found
}

enum RepositoryAIActionPresentation {
    static func commit(
        isDirty: Bool?,
        isDetached: Bool?,
        isRepositoryBusy: Bool,
        hasRunningAction: Bool
    ) -> RepositoryAIActionAvailability {
        if hasRunningAction {
            return .disabled("Wait for the current AI repository action to finish.")
        }
        if isRepositoryBusy {
            return .disabled("Wait for the current repository action to finish.")
        }
        guard let isDirty, let isDetached else {
            return .disabled("Loading repository status.")
        }
        if isDetached {
            return .disabled("Switch to a branch before committing and pushing.")
        }
        guard isDirty else { return .disabled("The working tree is clean.") }
        return .available
    }

    static func createPullRequest(
        pullRequest: RepositoryPullRequestPresence,
        isDetached: Bool,
        isRepositoryBusy: Bool,
        hasRunningAction: Bool
    ) -> RepositoryAIActionAvailability {
        guard pullRequest == .none else { return .hidden }
        if hasRunningAction {
            return .disabled("Wait for the current AI repository action to finish.")
        }
        if isRepositoryBusy {
            return .disabled("Wait for the current repository action to finish.")
        }
        if isDetached {
            return .disabled("Switch to a branch before creating a pull request.")
        }
        return .available
    }
}

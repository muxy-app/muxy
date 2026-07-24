import Foundation

struct RepositoryAIActionConfirmation: Equatable {
    struct Context: Equatable {
        let repositoryID: String
        let branch: String
    }

    let action: RepositoryAIAction
    let context: Context
    let title: String
    let message: String
    let confirmTitle: String

    init(
        action: RepositoryAIAction,
        context: Context,
        providerName: String
    ) {
        self.action = action
        self.context = context
        confirmTitle = action.settingsTitle
        switch action {
        case .commit:
            title = "Commit and push to \"\(context.branch)\"?"
            message = "Muxy will stage all changes, ask \(providerName) for a commit message, "
                + "then commit and push to \"\(context.branch)\"."
        case .createPullRequest:
            title = "Create a pull request from \"\(context.branch)\"?"
            message = "Muxy will stage all changes, ask \(providerName) for a branch name, title, and summary, "
                + "then create the branch, commit, push, and open the pull request on GitHub."
        }
    }
}

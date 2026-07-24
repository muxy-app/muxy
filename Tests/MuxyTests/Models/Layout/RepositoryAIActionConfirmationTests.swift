import Testing

@testable import Muxy

@Suite("RepositoryAIActionConfirmation")
struct RepositoryAIActionConfirmationTests {
    private func makeConfirmation(
        action: RepositoryAIAction,
        branch: String = "main",
        providerName: String = "Claude Code"
    ) -> RepositoryAIActionConfirmation {
        RepositoryAIActionConfirmation(
            action: action,
            context: RepositoryAIActionConfirmation.Context(
                repositoryID: "project|worktree|/repo",
                branch: branch
            ),
            providerName: providerName
        )
    }

    @Test("commit confirmation names the branch, the provider, and the push")
    func commitConfirmationDescribesTheWorkflow() {
        let confirmation = makeConfirmation(action: .commit)

        #expect(confirmation.title == "Commit and push to \"main\"?")
        #expect(confirmation.message == "Muxy will stage all changes, ask Claude Code for a commit message, "
            + "then commit and push to \"main\".")
        #expect(confirmation.confirmTitle == "Commit and Push")
    }

    @Test("create pull request confirmation names the branch, the provider, and GitHub")
    func createPullRequestConfirmationDescribesTheWorkflow() {
        let confirmation = makeConfirmation(action: .createPullRequest, branch: "feature")

        #expect(confirmation.title == "Create a pull request from \"feature\"?")
        #expect(confirmation.message == "Muxy will stage all changes, ask Claude Code for a branch name, "
            + "title, and summary, then create the branch, commit, push, and open the pull request on GitHub.")
        #expect(confirmation.confirmTitle == "Create Pull Request")
    }

    @Test("every action credits Muxy with the git workflow and the provider only with generated text")
    func messagesAttributeTheGitWorkflowToMuxy() {
        for action in RepositoryAIAction.allCases {
            let confirmation = makeConfirmation(action: action, providerName: "Codex")

            #expect(confirmation.message.hasPrefix("Muxy will stage all changes, ask Codex for "))
        }
    }
}

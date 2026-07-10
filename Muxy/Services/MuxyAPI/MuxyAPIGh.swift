import Foundation

extension MuxyAPI {
    @MainActor
    enum Gh {
        struct Context {
            let extensionID: String
            let appState: AppState
            let projectStore: ProjectStore
            let worktreeStore: WorktreeStore
            let projectGroupStore: ProjectGroupStore
        }

        struct GitHubUser {
            let login: String
            let name: String
            let avatarUrl: String
        }

        private static var cachedUser: GitHubUser?
        private static var cachedUserAt: Date?
        private static let userTTL: TimeInterval = 300

        static func user(context: Context) async -> Result<GitHubUser, APIError> {
            if let cached = cachedUser, let at = cachedUserAt,
               Date().timeIntervalSince(at) < userTTL
            {
                return .success(cached)
            }

            guard let ghPath = GitProcessRunner.resolveExecutable("gh") else {
                return .failure(.underlying("GitHub CLI (gh) is not installed. Install it from cli.github.com."))
            }

            do {
                let result = try await GitProcessRunner.runCommand(
                    executable: ghPath,
                    arguments: ["api", "user", "--jq", "{login: .login, name: .name, avatarUrl: .avatar_url}"],
                    workingDirectory: ""
                )
                guard result.status == 0 else {
                    let msg = result.stderr.isEmpty ? result.stdout : result.stderr
                    return .failure(.underlying(msg.trimmingCharacters(in: .whitespacesAndNewlines)))
                }
                guard let data = result.stdout.data(using: .utf8),
                      let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                      let login = json["login"] as? String
                else {
                    return .failure(.underlying("Failed to parse GitHub user info."))
                }
                let user = GitHubUser(
                    login: login,
                    name: json["name"] as? String ?? "",
                    avatarUrl: json["avatarUrl"] as? String ?? ""
                )
                cachedUser = user
                cachedUserAt = Date()
                return .success(user)
            } catch {
                return .failure(.underlying(error.localizedDescription))
            }
        }
    }
}

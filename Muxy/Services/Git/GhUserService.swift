import Foundation

struct GhUser: Equatable {
    let login: String
    let name: String
    let avatarUrl: String
}

enum GhUserError: Error {
    case notInstalled
    case command(String)
    case parseFailed

    var message: String {
        switch self {
        case .notInstalled: "GitHub CLI (gh) is not installed. Install it from cli.github.com."
        case let .command(detail): detail
        case .parseFailed: "Failed to parse GitHub user info."
        }
    }
}

actor GhUserService {
    static let shared = GhUserService()

    private let ttl: TimeInterval = 300
    private var cachedUser: GhUser?
    private var cachedAt: Date?

    func user(now: Date = Date()) async -> Result<GhUser, GhUserError> {
        if let cachedUser, let cachedAt, now.timeIntervalSince(cachedAt) < ttl {
            return .success(cachedUser)
        }

        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else {
            return .failure(.notInstalled)
        }

        do {
            let result = try await GitProcessRunner.runCommand(
                executable: ghPath,
                arguments: ["api", "user", "--jq", "{login: .login, name: .name, avatarUrl: .avatar_url}"],
                workingDirectory: NSHomeDirectory()
            )
            guard result.status == 0 else {
                let output = result.stderr.isEmpty ? result.stdout : result.stderr
                return .failure(.command(output.trimmingCharacters(in: .whitespacesAndNewlines)))
            }
            guard let data = result.stdout.data(using: .utf8),
                  let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let login = json["login"] as? String
            else {
                return .failure(.parseFailed)
            }
            let user = GhUser(
                login: login,
                name: json["name"] as? String ?? "",
                avatarUrl: json["avatarUrl"] as? String ?? ""
            )
            cachedUser = user
            cachedAt = now
            return .success(user)
        } catch {
            return .failure(.command(error.localizedDescription))
        }
    }
}

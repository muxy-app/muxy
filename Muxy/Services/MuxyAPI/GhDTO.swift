import Foundation

enum GhDTO {
    static func user(_ user: MuxyAPI.Gh.GitHubUser) -> [String: Any] {
        [
            "login": user.login,
            "name": user.name,
            "avatarUrl": user.avatarUrl,
        ]
    }
}

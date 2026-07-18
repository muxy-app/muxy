import Foundation

enum GhDTO {
    static func user(_ user: GhUser) -> [String: Any] {
        [
            "login": user.login,
            "name": user.name,
            "avatarUrl": user.avatarUrl,
        ]
    }
}

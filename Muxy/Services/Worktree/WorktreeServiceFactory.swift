import Foundation

enum WorktreeServiceFactory {
    static func service(for path: String) async -> any WorktreeService {
        let kind = await VCSKind.detect(at: path)
        if kind?.isJujutsu ?? false {
            return JJWorktreeService.shared
        }
        return GitWorktreeService.shared
    }

    static func isRepository(_ path: String) async -> Bool {
        let kind = await VCSKind.detect(at: path)
        return kind != nil
    }
}

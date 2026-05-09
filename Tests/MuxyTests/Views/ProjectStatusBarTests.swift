import Testing

@testable import Muxy

@Suite("ProjectStatusBar")
@MainActor
struct ProjectStatusBarTests {
    @Test("short paths are returned unchanged")
    func shortPathUnchanged() {
        let path = "~/Projects/muxy"
        #expect(ProjectStatusBar.truncatePath(path, maxCharacters: 40) == path)
    }

    @Test("long paths keep the trailing portion with leading ellipsis")
    func longPathTruncatedFromStart() {
        let path = "~/Projects/muxy/worktree-checkouts/some-very-long-feature-branch/sources"
        let result = ProjectStatusBar.truncatePath(path, maxCharacters: 40)
        #expect(result.count == 40)
        #expect(result.hasPrefix("…"))
        #expect(path.hasSuffix(String(result.dropFirst())))
    }

    @Test("path exactly at the limit is unchanged")
    func pathAtBoundary() {
        let path = String(repeating: "a", count: 40)
        #expect(ProjectStatusBar.truncatePath(path, maxCharacters: 40) == path)
    }
}

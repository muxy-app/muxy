import Foundation
import Testing

@testable import Muxy

@Suite("MuxyAPI.Git permissions and DTOs")
struct MuxyAPIGitTests {
    @Test("read verbs require git:read")
    func readVerbsRequireGitRead() {
        for verb in ["git.status", "git.diff", "git.log", "git.branches", "git.pr.info", "git.worktrees"] {
            #expect(MuxyAPI.Permissions.required(for: verb) == .gitRead, "\(verb) should need git:read")
        }
    }

    @Test("write verbs require git:write")
    func writeVerbsRequireGitWrite() {
        for verb in ["git.commit", "git.push", "git.pr.create", "git.branch.create", "git.worktree.add"] {
            #expect(MuxyAPI.Permissions.required(for: verb) == .gitWrite, "\(verb) should need git:write")
        }
    }

    @Test("git verbs are recognized command names")
    func gitVerbsAreKnown() {
        #expect(MuxyAPI.Permissions.verbNames.contains("git.status"))
        #expect(MuxyAPI.Permissions.verbNames.contains("git.pr.merge"))
    }

    @Test("file DTO encodes status and staged flags")
    func fileDTOEncodesStatus() {
        let file = GitStatusFile(
            path: "a.txt",
            oldPath: nil,
            xStatus: "M",
            yStatus: " ",
            additions: 3,
            deletions: 1,
            stagedAdditions: 3,
            stagedDeletions: 1,
            unstagedAdditions: nil,
            unstagedDeletions: nil,
            isBinary: false
        )
        let dto = GitDTO.file(file, staged: true)
        #expect(dto["path"] as? String == "a.txt")
        #expect(dto["status"] as? String == "M")
        #expect(dto["isStaged"] as? Bool == true)
        #expect(dto["additions"] as? Int == 3)
    }

    @Test("ahead/behind DTO encodes counts")
    func aheadBehindDTO() {
        let value = GitRepositoryService.AheadBehind(ahead: 2, behind: 1, hasUpstream: true)
        let dto = GitDTO.aheadBehind(value)
        #expect(dto["ahead"] as? Int == 2)
        #expect(dto["behind"] as? Int == 1)
        #expect(dto["hasUpstream"] as? Bool == true)
    }
}

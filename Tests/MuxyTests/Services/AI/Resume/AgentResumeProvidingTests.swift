import Foundation
import Testing

@testable import Muxy

@Suite("AgentResumeProviding defaults")
struct AgentResumeProvidingTests {
    private struct BareProvider: AgentResumeProviding {}

    @Test("a provider with no strategy exposes nil seams")
    func defaultsAreNil() {
        let provider = BareProvider()
        #expect(provider.resumeStrategy == nil)
        #expect(provider.sessionStore == nil)
    }

    @Test("AgentSessionRef is value-equal")
    func sessionRefEquatable() {
        let date = Date(timeIntervalSince1970: 0)
        let a = AgentSessionRef(id: "1", providerID: "claude", cwd: "/x", gitBranch: nil,
                                title: nil, preview: nil, updatedAt: date, pinned: false, archived: false)
        let b = AgentSessionRef(id: "1", providerID: "claude", cwd: "/x", gitBranch: nil,
                                title: nil, preview: nil, updatedAt: date, pinned: false, archived: false)
        #expect(a == b)
    }
}

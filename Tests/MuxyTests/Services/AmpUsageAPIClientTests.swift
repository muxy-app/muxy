import Foundation
import Testing

@testable import Muxy

@Suite("AmpUsageProvider")
struct AmpUsageProviderTests {
    @Test("reads token from env")
    func readTokenFromEnv() throws {
        let token = try AmpUsageProvider.readToken(
            env: ["AMP_API_KEY": "amp_token"],
            homeDirectory: "/mock-home",
            fileExists: { _ in false },
            dataReader: { _ in throw NSError(domain: "test", code: 1) }
        )
        #expect(token == "amp_token")
    }

    @Test("reads token from secrets.json fallback")
    func readTokenFromSecretsFile() throws {
        let home = "/mock-home"
        let path = home + "/.local/share/amp/secrets.json"
        let payload = Data(#"{"apiKey":"amp_from_disk"}"#.utf8)

        let token = try AmpUsageProvider.readToken(
            env: [:],
            homeDirectory: home,
            fileExists: { $0 == path },
            dataReader: { requested in
                guard requested == path else { throw NSError(domain: "test", code: 1) }
                return payload
            }
        )
        #expect(token == "amp_from_disk")
    }
}

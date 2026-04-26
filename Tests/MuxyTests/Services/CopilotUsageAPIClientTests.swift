import Foundation
import Testing

@testable import Muxy

@Suite("CopilotUsageProvider")
struct CopilotUsageProviderTests {
    @Test("reads token from gh CLI keychain base64 payload")
    func readTokenFromGhKeychainBase64() throws {
        let encoded = Data("gho_decoded".utf8).base64EncodedString()

        let token = try CopilotUsageProvider.readToken(
            env: [:],
            keychainReader: { service in
                if service == "gh:github.com" {
                    return "go-keyring-base64:\(encoded)"
                }
                return nil
            },
            fileExists: { _ in false },
            dataReader: { _ in throw NSError(domain: "test", code: 1) }
        )

        #expect(token == "gho_decoded")
    }

    @Test("reads token from gh hosts.yml fallback")
    func readTokenFromGhHostsYAMLFallback() throws {
        let home = "/mock-home"
        let yamlPath = home + "/.config/gh/hosts.yml"
        let files = [
            yamlPath: Data(
                """
                github.com:
                  user: octocat
                  oauth_token: ghu_yaml
                """.utf8
            )
        ]

        let token = try CopilotUsageProvider.readToken(
            env: [:],
            keychainReader: { _ in nil },
            homeDirectory: home,
            fileExists: { path in files[path] != nil },
            dataReader: { path in
                guard let data = files[path] else { throw NSError(domain: "test", code: 1) }
                return data
            }
        )

        #expect(token == "ghu_yaml")
    }
}

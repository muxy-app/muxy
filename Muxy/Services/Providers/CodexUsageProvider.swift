import Foundation

struct CodexUsageProvider: AIUsageProvider {
    let id = "codex"
    let displayName = "Codex"
    let iconName = "codex"

    private static let endpoint = URL(string: "https://chatgpt.com/backend-api/wham/usage")

    func fetchUsageSnapshot() async -> AIProviderUsageSnapshot {
        await AIUsageSession.fetchSnapshot(
            provider: self,
            messages: AIUsageSessionMessages(
                missingCredentials: "Sign in to Codex",
                unauthenticated: "Sign in to Codex"
            ),
            buildRequest: {
                guard let endpoint = Self.endpoint else { throw AIUsageAuthError.missingCredentials }
                let auth = try Self.readAuth()
                var request = URLRequest(url: endpoint)
                request.httpMethod = "GET"
                request.setValue("Bearer \(auth.accessToken)", forHTTPHeaderField: "Authorization")
                request.setValue("application/json", forHTTPHeaderField: "Accept")
                if let accountID = auth.accountID, !accountID.isEmpty {
                    request.setValue(accountID, forHTTPHeaderField: "ChatGPT-Account-Id")
                }
                return request
            },
            parse: CodexUsageParser.parseMetricRows(from:)
        )
    }

    static func readAuth(
        env: [String: String] = ProcessInfo.processInfo.environment,
        homeDirectory: String = NSHomeDirectory(),
        fileExists: ((String) -> Bool)? = nil,
        dataReader: ((String) throws -> Data)? = nil
    ) throws -> (accessToken: String, accountID: String?) {
        if let token = AIUsageTokenReader.fromEnvironment(keys: ["CODEX_ACCESS_TOKEN"], env: env) {
            return (token, env["CODEX_ACCOUNT_ID"]?.trimmingCharacters(in: .whitespacesAndNewlines))
        }

        let doesFileExist = fileExists ?? { FileManager.default.fileExists(atPath: $0) }
        let readData = dataReader ?? { try Data(contentsOf: URL(fileURLWithPath: $0)) }

        let candidatePaths: [String] = [
            env["CODEX_HOME"].map { "\($0)/auth.json" },
            "\(homeDirectory)/.config/codex/auth.json",
            "\(homeDirectory)/.codex/auth.json",
        ].compactMap(\.self)

        for path in candidatePaths {
            guard doesFileExist(path) else { continue }
            let data = try readData(path)
            guard let payload = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { continue }

            if let tokens = payload["tokens"] as? [String: Any],
               let accessToken = AIUsageParserSupport.string(in: tokens, keys: ["access_token"]),
               !accessToken.isEmpty
            {
                let accountID = AIUsageParserSupport.string(in: tokens, keys: ["account_id"])
                return (accessToken, accountID)
            }
        }

        throw AIUsageAuthError.missingCredentials
    }
}

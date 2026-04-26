import Foundation

struct AmpUsageProvider: AIUsageProvider {
    let id = "amp"
    let displayName = "Amp"
    let iconName = "amp"

    private static let endpoint = URL(string: "https://ampcode.com/api/internal")

    func fetchUsageSnapshot() async -> AIProviderUsageSnapshot {
        await AIUsageSession.fetchSnapshot(
            provider: self,
            messages: AIUsageSessionMessages(
                missingCredentials: "Sign in to Amp",
                unauthenticated: "Session expired. Re-authenticate in Amp."
            ),
            buildRequest: {
                guard let endpoint = Self.endpoint else { throw AIUsageAuthError.missingCredentials }
                let token = try Self.readToken()
                var request = URLRequest(url: endpoint)
                request.httpMethod = "POST"
                request.setValue("application/json", forHTTPHeaderField: "Content-Type")
                request.setValue("application/json", forHTTPHeaderField: "Accept")
                request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
                request.httpBody = try JSONSerialization.data(withJSONObject: [
                    "method": "userDisplayBalanceInfo",
                    "params": [:],
                ])
                return request
            },
            parse: AmpUsageParser.parseMetricRows(from:)
        )
    }

    static func readToken(
        env: [String: String] = ProcessInfo.processInfo.environment,
        homeDirectory: String = NSHomeDirectory(),
        fileExists: ((String) -> Bool)? = nil,
        dataReader: ((String) throws -> Data)? = nil
    ) throws -> String {
        if let token = AIUsageTokenReader.fromEnvironment(keys: ["AMP_API_KEY"], env: env) {
            return token
        }

        let doesFileExist = fileExists ?? { FileManager.default.fileExists(atPath: $0) }
        let readData = dataReader ?? { try Data(contentsOf: URL(fileURLWithPath: $0)) }

        let path = homeDirectory + "/.local/share/amp/secrets.json"
        if doesFileExist(path) {
            let data = try readData(path)
            if let payload = try JSONSerialization.jsonObject(with: data) as? [String: Any],
               let token = AIUsageParserSupport.string(
                   in: payload,
                   keys: ["apiKey@https://ampcode.com/", "apiKey", "token"]
               ),
               !token.isEmpty
            {
                return token
            }
        }

        throw AIUsageAuthError.missingCredentials
    }
}

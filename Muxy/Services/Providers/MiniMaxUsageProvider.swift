import Foundation

enum MiniMaxRegion {
    case global
    case cn
}

struct MiniMaxUsageProvider: AIUsageProvider {
    let id = "minimax"
    let displayName = "MiniMax"
    let iconName = "minimax"

    private static let globalEndpoints: [URL] = [
        URL(string: "https://api.minimax.io/v1/api/openplatform/coding_plan/remains"),
        URL(string: "https://api.minimax.io/v1/coding_plan/remains"),
        URL(string: "https://www.minimax.io/v1/api/openplatform/coding_plan/remains"),
    ].compactMap(\.self)

    private static let cnEndpoints: [URL] = [
        URL(string: "https://api.minimaxi.com/v1/api/openplatform/coding_plan/remains"),
        URL(string: "https://api.minimaxi.com/v1/coding_plan/remains"),
    ].compactMap(\.self)

    func fetchUsageSnapshot() async -> AIProviderUsageSnapshot {
        do {
            let credentials = try Self.readCredentials()
            let regionAttempts = Self.buildRegionAttempts(credentials: credentials)
            guard !regionAttempts.isEmpty else {
                throw ClientError.missingAPIKey
            }

            var firstError: Error?
            for attempt in regionAttempts {
                do {
                    let rows = try await Self.fetchRows(for: attempt)
                    guard !rows.isEmpty else {
                        firstError = firstError ?? ClientError.noUsageData
                        continue
                    }
                    return snapshot(state: .available, rows: rows)
                } catch {
                    firstError = firstError ?? error
                }
            }

            return errorSnapshot(for: firstError ?? ClientError.noUsageData)
        } catch {
            return errorSnapshot(for: error)
        }
    }

    static func readToken(
        env: [String: String] = ProcessInfo.processInfo.environment,
        homeDirectory: String = NSHomeDirectory(),
        fileExists: ((String) -> Bool)? = nil,
        dataReader: ((String) throws -> Data)? = nil
    ) throws -> String {
        let credentials = try readCredentials(
            env: env,
            homeDirectory: homeDirectory,
            fileExists: fileExists,
            dataReader: dataReader
        )
        if let token = token(for: preferredRegion(env: credentials.environment), credentials: credentials) {
            return token
        }
        throw ClientError.missingAPIKey
    }

    enum ClientError: Error {
        case missingAPIKey
        case sessionExpired
        case httpStatus(Int)
        case networkFailure
        case parseFailure
        case noUsageData
        case apiError(String)
    }

    private func errorSnapshot(for error: Error) -> AIProviderUsageSnapshot {
        switch error {
        case ClientError.missingAPIKey:
            return snapshot(state: .unavailable(message: "MiniMax API key missing. Set MINIMAX_API_KEY or MINIMAX_CN_API_KEY."))
        case ClientError.sessionExpired:
            return snapshot(state: .unavailable(message: "Session expired. Check your MiniMax API key."))
        case let ClientError.httpStatus(code):
            usageLogger.error("MiniMax usage request failed with status \(code)")
            return snapshot(state: .error(message: "Request failed (HTTP \(code)). Try again later."))
        case ClientError.networkFailure:
            return snapshot(state: .error(message: "Request failed. Check your connection."))
        case ClientError.parseFailure:
            return snapshot(state: .error(message: "Could not parse usage data."))
        case ClientError.noUsageData:
            return snapshot(state: .unavailable(message: "No usage data"))
        case let ClientError.apiError(message):
            return snapshot(state: .error(message: "MiniMax API error: \(message)"))
        case let parserError as MiniMaxUsageParserError:
            switch parserError {
            case let .apiError(message):
                return errorSnapshot(for: ClientError.apiError(message))
            case .authError:
                return errorSnapshot(for: ClientError.sessionExpired)
            case .invalidPayload:
                return errorSnapshot(for: ClientError.parseFailure)
            }
        case let nsError as NSError where nsError.domain == NSURLErrorDomain:
            return errorSnapshot(for: ClientError.networkFailure)
        default:
            usageLogger.error("MiniMax usage request failed: \(error.localizedDescription)")
            return snapshot(state: .error(message: "Unable to fetch usage"))
        }
    }

    private static func fetchRows(for attempt: (region: MiniMaxRegion, token: String)) async throws -> [AIUsageMetricRow] {
        var hadNetworkError = false
        var authStatusCount = 0
        var lastStatusCode: Int?
        var parsedEmpty = false

        for endpoint in endpoints(for: attempt.region) {
            do {
                var request = URLRequest(url: endpoint)
                request.httpMethod = "GET"
                request.setValue("Bearer \(attempt.token)", forHTTPHeaderField: "Authorization")
                request.setValue("application/json", forHTTPHeaderField: "Accept")
                request.setValue("application/json", forHTTPHeaderField: "Content-Type")

                let (data, response) = try await URLSession.shared.data(for: request)
                guard let http = response as? HTTPURLResponse else { continue }

                if http.statusCode == 401 || http.statusCode == 403 {
                    authStatusCount += 1
                    continue
                }
                guard (200 ..< 300).contains(http.statusCode) else {
                    lastStatusCode = http.statusCode
                    continue
                }

                do {
                    let rows = try MiniMaxUsageParser.parseMetricRows(from: data, region: attempt.region)
                    if !rows.isEmpty { return rows }
                    parsedEmpty = true
                    continue
                } catch let parserError as MiniMaxUsageParserError {
                    switch parserError {
                    case .authError: throw ClientError.sessionExpired
                    case let .apiError(message): throw ClientError.apiError(message)
                    case .invalidPayload: continue
                    }
                }
            } catch let nsError as NSError where nsError.domain == NSURLErrorDomain {
                hadNetworkError = true
                continue
            }
        }

        if authStatusCount > 0, lastStatusCode == nil, !hadNetworkError { throw ClientError.sessionExpired }
        if let lastStatusCode { throw ClientError.httpStatus(lastStatusCode) }
        if hadNetworkError { throw ClientError.networkFailure }
        if parsedEmpty { throw ClientError.noUsageData }
        throw ClientError.parseFailure
    }

    private static func readCredentials(
        env: [String: String] = ProcessInfo.processInfo.environment,
        homeDirectory: String = NSHomeDirectory(),
        fileExists: ((String) -> Bool)? = nil,
        dataReader: ((String) throws -> Data)? = nil
    ) throws -> (environment: [String: String], fallbackToken: String?) {
        let hasEnvToken = AIUsageTokenReader.fromEnvironment(
            keys: ["MINIMAX_CN_API_KEY", "MINIMAX_API_KEY", "MINIMAX_API_TOKEN"],
            env: env
        ) != nil

        let fallbackToken = readFallbackTokenFromDisk(
            homeDirectory: homeDirectory,
            fileExists: fileExists ?? { FileManager.default.fileExists(atPath: $0) },
            dataReader: dataReader ?? { try Data(contentsOf: URL(fileURLWithPath: $0)) }
        )

        if hasEnvToken || fallbackToken != nil {
            return (env, fallbackToken)
        }
        throw ClientError.missingAPIKey
    }

    private static func readFallbackTokenFromDisk(
        homeDirectory: String,
        fileExists: (String) -> Bool,
        dataReader: (String) throws -> Data
    ) -> String? {
        for path in ["\(homeDirectory)/.mmx/config.json", "\(homeDirectory)/.mmx/credentials.json"] {
            guard fileExists(path), let data = try? dataReader(path) else { continue }
            guard let payload = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { continue }
            if let token = AIUsageParserSupport.string(
                in: payload,
                keys: ["api_key", "apiKey", "token", "access_token"]
            ), !token.isEmpty {
                return token
            }
            if let auth = payload["auth"] as? [String: Any],
               let token = AIUsageParserSupport.string(
                   in: auth,
                   keys: ["api_key", "apiKey", "token", "access_token"]
               ),
               !token.isEmpty
            {
                return token
            }
        }
        return nil
    }

    private static func preferredRegion(env: [String: String]) -> MiniMaxRegion {
        AIUsageTokenReader.fromEnvironment(keys: ["MINIMAX_CN_API_KEY"], env: env) != nil ? .cn : .global
    }

    private static func regionOrder(env: [String: String]) -> [MiniMaxRegion] {
        preferredRegion(env: env) == .cn ? [.cn, .global] : [.global, .cn]
    }

    private static func token(
        for region: MiniMaxRegion,
        credentials: (environment: [String: String], fallbackToken: String?)
    ) -> String? {
        let env = credentials.environment
        switch region {
        case .global:
            return AIUsageTokenReader.fromEnvironment(keys: ["MINIMAX_API_KEY", "MINIMAX_API_TOKEN"], env: env)
                ?? credentials.fallbackToken
        case .cn:
            return AIUsageTokenReader.fromEnvironment(
                keys: ["MINIMAX_CN_API_KEY", "MINIMAX_API_KEY", "MINIMAX_API_TOKEN"],
                env: env
            ) ?? credentials.fallbackToken
        }
    }

    private static func endpoints(for region: MiniMaxRegion) -> [URL] {
        switch region {
        case .global: globalEndpoints
        case .cn: cnEndpoints
        }
    }

    private static func buildRegionAttempts(
        credentials: (environment: [String: String], fallbackToken: String?)
    ) -> [(region: MiniMaxRegion, token: String)] {
        regionOrder(env: credentials.environment).compactMap { region in
            guard let token = token(for: region, credentials: credentials) else { return nil }
            return (region: region, token: token)
        }
    }
}

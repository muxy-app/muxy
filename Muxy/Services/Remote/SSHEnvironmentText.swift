import Foundation

enum SSHEnvironmentText {
    static func format(_ environment: [String: String]) -> String {
        environment
            .sorted { $0.key < $1.key }
            .map { "\($0.key)=\($0.value)" }
            .joined(separator: "\n")
    }

    static func parse(_ text: String) -> Result<[String: String], SSHEnvironmentTextError> {
        var environment: [String: String] = [:]
        var seen: Set<String> = []
        let lines = text.components(separatedBy: .newlines)
        for (index, rawLine) in lines.enumerated() {
            guard !rawLine.trimmingCharacters(in: .whitespaces).isEmpty else { continue }
            guard let separator = rawLine.firstIndex(of: "=") else {
                return .failure(.missingAssignment(line: index + 1))
            }
            let key = rawLine[..<separator].trimmingCharacters(in: .whitespaces)
            let value = String(rawLine[rawLine.index(after: separator)...])
            guard RemoteCommandBuilder.isValidEnvironmentKey(key) else {
                return .failure(.invalidKey(String(key)))
            }
            guard !seen.contains(String(key)) else {
                return .failure(.duplicateKey(String(key)))
            }
            seen.insert(String(key))
            environment[String(key)] = value
        }
        return .success(environment)
    }
}

enum SSHEnvironmentTextError: Error, Equatable, LocalizedError {
    case missingAssignment(line: Int)
    case invalidKey(String)
    case duplicateKey(String)

    var errorDescription: String? {
        switch self {
        case let .missingAssignment(line):
            "Environment line \(line) must use KEY=value."
        case let .invalidKey(key):
            "Environment key '\(key)' is invalid."
        case let .duplicateKey(key):
            "Environment key '\(key)' is duplicated."
        }
    }
}

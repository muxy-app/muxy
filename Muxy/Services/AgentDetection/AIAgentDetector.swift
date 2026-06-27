import Foundation

struct AIAgentExecutable: Equatable {
    let providerID: String
    let executableNames: [String]
}

enum AIAgentDetector {
    static func providerID(
        forProcessName processName: String?,
        executables: [AIAgentExecutable]
    ) -> String? {
        guard let normalized = normalize(processName) else { return nil }
        for executable in executables {
            for name in executable.executableNames where name.lowercased() == normalized {
                return executable.providerID
            }
        }
        return nil
    }

    private static func normalize(_ processName: String?) -> String? {
        guard let processName else { return nil }
        let trimmed = processName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let firstToken = trimmed.split(separator: " ", maxSplits: 1).first.map(String.init) ?? trimmed
        let basename = (firstToken as NSString).lastPathComponent
        let withoutLeadingDash = basename.hasPrefix("-") ? String(basename.dropFirst()) : basename
        guard !withoutLeadingDash.isEmpty else { return nil }
        return withoutLeadingDash.lowercased()
    }
}

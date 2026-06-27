import Foundation

struct AIAgentExecutable: Equatable {
    let providerID: String
    let executableNames: [String]
}

enum AIAgentDetector {
    static func providerID(
        forCandidateNames candidateNames: [String],
        executables: [AIAgentExecutable]
    ) -> String? {
        let normalizedCandidates = candidateNames.compactMap(normalize)
        guard !normalizedCandidates.isEmpty else { return nil }
        for executable in executables {
            for name in executable.executableNames {
                let normalizedName = name.lowercased()
                if normalizedCandidates.contains(normalizedName) {
                    return executable.providerID
                }
            }
        }
        return nil
    }

    static func providerID(
        forProcessName processName: String?,
        executables: [AIAgentExecutable]
    ) -> String? {
        guard let processName else { return nil }
        return providerID(forCandidateNames: [processName], executables: executables)
    }

    private static func normalize(_ name: String) -> String? {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let firstToken = trimmed.split(separator: " ", maxSplits: 1).first.map(String.init) ?? trimmed
        let basename = (firstToken as NSString).lastPathComponent
        let withoutLeadingDash = basename.hasPrefix("-") ? String(basename.dropFirst()) : basename
        guard !withoutLeadingDash.isEmpty else { return nil }
        return withoutLeadingDash.lowercased()
    }
}

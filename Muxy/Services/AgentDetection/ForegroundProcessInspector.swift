import Foundation

enum ForegroundProcessInspector {
    private static let scriptInterpreters: Set<String> = [
        "node",
        "bun",
        "deno",
        "python",
        "python3",
        "ruby",
        "perl",
        "sh",
        "bash",
        "zsh",
    ]

    static func executableNameCandidates(pid: UInt64) -> [String] {
        guard let invocation = ProcessArgumentsInspector.invocation(pid: pid) else { return [] }

        var candidates: [String] = []
        let executableName = lastPathComponent(of: invocation.executablePath)
        if let executableName {
            candidates.append(executableName)
        }

        let argumentNames = invocation.arguments.compactMap(lastPathComponent)
        if let firstArgument = argumentNames.first {
            candidates.append(firstArgument)
        }

        if isScriptInterpreter(executableName: executableName, firstArgument: argumentNames.first),
           argumentNames.count > 1
        {
            candidates.append(argumentNames[1])
        }
        return candidates
    }

    private static func isScriptInterpreter(executableName: String?, firstArgument: String?) -> Bool {
        if let executableName, scriptInterpreters.contains(executableName) {
            return true
        }
        if let firstArgument, scriptInterpreters.contains(firstArgument) {
            return true
        }
        return false
    }

    private static func lastPathComponent(of path: String) -> String? {
        let trimmed = path.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let component = (trimmed as NSString).lastPathComponent
        return component.isEmpty ? nil : component
    }
}

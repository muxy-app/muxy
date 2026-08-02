import Foundation

enum SSHInvocationParser {
    private static let optionsTakingValue: Set<Character> = [
        "B", "b", "c", "D", "E", "e", "F", "I", "i", "J", "L", "l", "m",
        "O", "P", "p", "Q", "R", "S", "W", "w", "o",
    ]

    private static let unreproducibleOptions: Set<Character> = ["J", "F", "W"]

    private static let unreproducibleKeywords = [
        "proxyjump",
        "proxycommand",
        "remotecommand",
    ]

    static func destination(from invocation: ProcessInvocation) -> SSHDestination? {
        guard isSSHExecutable(invocation) else { return nil }
        return destination(arguments: Array(invocation.arguments.dropFirst()))
    }

    static func destination(arguments: [String]) -> SSHDestination? {
        var target: String?
        var user: String?
        var port: Int?
        var identityFile: String?
        var index = 0

        while index < arguments.count {
            let argument = arguments[index]
            index += 1

            if target != nil {
                continue
            }
            if argument == "--" {
                continue
            }
            guard argument.hasPrefix("-"), argument.count > 1 else {
                target = argument
                continue
            }

            var characters = Array(argument.dropFirst())
            var characterIndex = 0
            while characterIndex < characters.count {
                let flag = characters[characterIndex]
                characterIndex += 1
                guard optionsTakingValue.contains(flag) else {
                    if unreproducibleOptions.contains(flag) {
                        return nil
                    }
                    continue
                }
                if unreproducibleOptions.contains(flag) {
                    return nil
                }

                let inlineValue = String(characters[characterIndex...])
                let value: String
                if inlineValue.isEmpty {
                    guard index < arguments.count else { return nil }
                    value = arguments[index]
                    index += 1
                } else {
                    value = inlineValue
                }
                characterIndex = characters.count

                switch flag {
                case "p":
                    guard let parsed = Int(value), (1 ... 65535).contains(parsed) else { return nil }
                    port = parsed
                case "l":
                    user = value
                case "i":
                    identityFile = value
                case "o":
                    guard isReproducibleOption(value) else { return nil }
                default:
                    continue
                }
            }
            characters = []
        }

        guard let target else { return nil }
        return destination(target: target, user: user, port: port, identityFile: identityFile)
    }

    static func isSSHExecutable(_ invocation: ProcessInvocation) -> Bool {
        let executableName = (invocation.executablePath as NSString).lastPathComponent
        guard executableName == "ssh" else { return false }
        return !invocation.arguments.isEmpty
    }

    private static func isReproducibleOption(_ value: String) -> Bool {
        let keyword = value.split(separator: "=", maxSplits: 1).first.map {
            $0.trimmingCharacters(in: .whitespaces).lowercased()
        }
        guard let keyword else { return true }
        return !unreproducibleKeywords.contains(keyword)
    }

    private static func destination(
        target: String,
        user: String?,
        port: Int?,
        identityFile: String?
    ) -> SSHDestination? {
        var remainder = target
        var resolvedUser = user
        var resolvedPort = port

        if remainder.lowercased().hasPrefix("ssh://") {
            guard let components = URLComponents(string: remainder), let componentHost = components.host else {
                return nil
            }
            remainder = componentHost
            resolvedUser = components.user ?? resolvedUser
            resolvedPort = components.port ?? resolvedPort
        } else if let separatorIndex = remainder.lastIndex(of: "@") {
            resolvedUser = String(remainder[remainder.startIndex ..< separatorIndex])
            remainder = String(remainder[remainder.index(after: separatorIndex)...])
        }

        guard SSHDestination.isValidHost(remainder), !remainder.contains("/") else { return nil }
        if let resolvedUser, resolvedUser.isEmpty {
            return nil
        }

        return SSHDestination(
            host: remainder,
            port: resolvedPort,
            user: resolvedUser,
            identityFile: identityFile
        )
    }
}

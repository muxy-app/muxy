import Foundation

enum SSHInvocationParser {
    private static let optionsTakingValue: Set<Character> = [
        "B", "b", "c", "D", "E", "e", "F", "I", "i", "J", "L", "l", "m",
        "O", "P", "p", "Q", "R", "S", "W", "w", "o",
    ]

    private static let supportedOptionsTakingValue: Set<Character> = ["i", "l", "o", "p"]
    private static let ignoredOptions: Set<Character> = ["C", "N", "n", "q", "T", "t", "v"]

    static func destination(from invocation: ProcessInvocation) -> SSHDestination? {
        guard isSSHExecutable(invocation) else { return nil }
        return destination(
            arguments: Array(invocation.arguments.dropFirst()),
            workingDirectory: invocation.workingDirectory
        )
    }

    static func destination(
        arguments: [String],
        workingDirectory: String? = nil
    ) -> SSHDestination? {
        var target: String?
        var user: String?
        var port: Int?
        var identityFile: String?
        var index = 0
        var acceptsOptions = true

        while index < arguments.count {
            let argument = arguments[index]
            index += 1

            guard target == nil else { return nil }
            if acceptsOptions, argument == "--" {
                acceptsOptions = false
                continue
            }
            guard acceptsOptions, argument.hasPrefix("-"), argument.count > 1 else {
                target = argument
                continue
            }

            let characters = Array(argument.dropFirst())
            var characterIndex = 0
            while characterIndex < characters.count {
                let flag = characters[characterIndex]
                characterIndex += 1
                guard optionsTakingValue.contains(flag) else {
                    guard ignoredOptions.contains(flag) else { return nil }
                    continue
                }
                guard supportedOptionsTakingValue.contains(flag) else { return nil }

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
                    guard let parsed = parsedPort(value) else { return nil }
                    port = port ?? parsed
                case "l":
                    user = user ?? value
                case "i":
                    guard applyIdentityFile(
                        value,
                        workingDirectory: workingDirectory,
                        identityFile: &identityFile
                    )
                    else { return nil }
                case "o":
                    guard applyOption(
                        value,
                        workingDirectory: workingDirectory,
                        user: &user,
                        port: &port,
                        identityFile: &identityFile
                    )
                    else { return nil }
                default:
                    continue
                }
            }
        }

        guard let target else { return nil }
        return destination(target: target, user: user, port: port, identityFile: identityFile)
    }

    static func isSSHExecutable(_ invocation: ProcessInvocation) -> Bool {
        let executableName = (invocation.executablePath as NSString).lastPathComponent
        guard executableName == "ssh" else { return false }
        return !invocation.arguments.isEmpty
    }

    private static func applyOption(
        _ option: String,
        workingDirectory: String?,
        user: inout String?,
        port: inout Int?,
        identityFile: inout String?
    ) -> Bool {
        guard let (keyword, value) = splitOption(option) else { return false }

        switch keyword {
        case "port":
            guard let parsed = parsedPort(value) else { return false }
            port = port ?? parsed
        case "user":
            guard !value.isEmpty else { return false }
            user = user ?? value
        case "identityfile":
            return applyIdentityFile(
                value,
                workingDirectory: workingDirectory,
                identityFile: &identityFile
            )
        default:
            return false
        }
        return true
    }

    private static func applyIdentityFile(
        _ value: String,
        workingDirectory: String?,
        identityFile: inout String?
    ) -> Bool {
        guard identityFile == nil,
              let resolved = resolvedIdentityFile(value, workingDirectory: workingDirectory)
        else { return false }
        identityFile = resolved
        return true
    }

    private static func resolvedIdentityFile(
        _ value: String,
        workingDirectory: String?
    ) -> String? {
        guard !value.isEmpty else { return nil }
        guard !value.contains("%"), !value.contains("${") else { return nil }
        if (value as NSString).isAbsolutePath
            || value.hasPrefix("~")
            || value.lowercased() == "none"
        {
            return value
        }
        guard let workingDirectory, (workingDirectory as NSString).isAbsolutePath else {
            return nil
        }
        return (workingDirectory as NSString).appendingPathComponent(value)
    }

    private static func splitOption(_ option: String) -> (keyword: String, value: String)? {
        guard let separatorIndex = option.firstIndex(where: { $0 == "=" || $0.isWhitespace }) else {
            return nil
        }
        let keyword = option[option.startIndex ..< separatorIndex]
            .trimmingCharacters(in: .whitespaces)
            .lowercased()
        let value = option[option.index(after: separatorIndex)...]
            .trimmingCharacters(in: .whitespaces)
        guard !keyword.isEmpty else { return nil }
        return (keyword, value)
    }

    private static func parsedPort(_ value: String) -> Int? {
        guard let parsed = Int(value), (1 ... 65535).contains(parsed) else { return nil }
        return parsed
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

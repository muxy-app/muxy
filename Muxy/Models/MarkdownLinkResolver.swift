import Foundation

enum MarkdownLinkResolver {
    enum Resolution: Equatable {
        case external(URL)
        case internalFile(path: String, fragment: String?)
        case sameDocumentFragment(String)
        case unsupported
    }

    static func resolve(href: String, currentFilePath: String?, projectPath: String?) -> Resolution {
        let trimmed = href.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return .unsupported }

        if let fragment = sameDocumentFragment(from: trimmed) {
            return .sameDocumentFragment(fragment)
        }

        if let scheme = explicitScheme(in: trimmed) {
            return resolveExplicitScheme(scheme, href: trimmed, projectPath: projectPath)
        }

        guard let currentFilePath else { return .unsupported }

        let parts = splitPathAndFragment(trimmed)
        guard !parts.path.isEmpty else {
            if let fragment = parts.fragment, !fragment.isEmpty {
                return .sameDocumentFragment(fragment)
            }
            return .unsupported
        }

        let decodedPath = parts.path.removingPercentEncoding ?? parts.path
        let targetPath = resolveFilePath(decodedPath, currentFilePath: currentFilePath, projectPath: projectPath)
        guard pathIsAllowed(targetPath, projectPath: projectPath) else { return .unsupported }
        return .internalFile(path: targetPath, fragment: decodedFragment(parts.fragment))
    }

    private static func resolveExplicitScheme(_ scheme: String, href: String, projectPath: String?) -> Resolution {
        if ["http", "https", "mailto"].contains(scheme), let url = URL(string: href) {
            return .external(url)
        }

        guard scheme == "file", let url = URL(string: href), url.isFileURL else {
            return .unsupported
        }

        let path = url.standardizedFileURL.path
        guard pathIsAllowed(path, projectPath: projectPath) else { return .unsupported }
        return .internalFile(path: path, fragment: decodedFragment(url.fragment))
    }

    private static func sameDocumentFragment(from href: String) -> String? {
        guard href.hasPrefix("#") else { return nil }
        let rawFragment = String(href.dropFirst())
        guard !rawFragment.isEmpty else { return nil }
        return decodedFragment(rawFragment)
    }

    private static func explicitScheme(in href: String) -> String? {
        guard let colonIndex = href.firstIndex(of: ":") else { return nil }
        let scheme = String(href[..<colonIndex])
        guard isValidScheme(scheme) else { return nil }
        return scheme.lowercased()
    }

    private static func isValidScheme(_ scheme: String) -> Bool {
        guard let first = scheme.first, first.isLetter else { return false }
        return scheme.dropFirst().allSatisfy { character in
            character.isLetter || character.isNumber || character == "+" || character == "-" || character == "."
        }
    }

    private static func splitPathAndFragment(_ href: String) -> (path: String, fragment: String?) {
        let pathAndQuery: String
        let fragment: String?

        if let hashIndex = href.firstIndex(of: "#") {
            pathAndQuery = String(href[..<hashIndex])
            fragment = String(href[href.index(after: hashIndex)...])
        } else {
            pathAndQuery = href
            fragment = nil
        }

        let path: String = if let queryIndex = pathAndQuery.firstIndex(of: "?") {
            String(pathAndQuery[..<queryIndex])
        } else {
            pathAndQuery
        }

        return (path, fragment)
    }

    private static func resolveFilePath(_ path: String, currentFilePath: String, projectPath: String?) -> String {
        if path.hasPrefix("/") {
            guard let projectPath, !projectPath.isEmpty else {
                return URL(fileURLWithPath: path).standardizedFileURL.path
            }

            let relativePath = String(path.drop { $0 == "/" })
            return URL(fileURLWithPath: projectPath)
                .appendingPathComponent(relativePath)
                .standardizedFileURL
                .path
        }

        return URL(fileURLWithPath: currentFilePath)
            .deletingLastPathComponent()
            .appendingPathComponent(path)
            .standardizedFileURL
            .path
    }

    private static func pathIsAllowed(_ path: String, projectPath: String?) -> Bool {
        guard let projectPath, !projectPath.isEmpty else { return true }

        let resolvedPath = URL(fileURLWithPath: path).standardizedFileURL.path
        let resolvedProjectPath = URL(fileURLWithPath: projectPath).standardizedFileURL.path
        guard resolvedPath != resolvedProjectPath else { return true }
        return resolvedPath.hasPrefix(resolvedProjectPath + "/")
    }

    private static func decodedFragment(_ fragment: String?) -> String? {
        guard let fragment, !fragment.isEmpty else { return nil }
        return fragment.removingPercentEncoding ?? fragment
    }
}

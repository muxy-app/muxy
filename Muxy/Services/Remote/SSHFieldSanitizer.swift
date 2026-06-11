import Foundation

enum SSHFieldSanitizer {
    static func host(_ value: String) -> String {
        stripLeadingDashes(value.trimmingCharacters(in: .whitespacesAndNewlines))
    }

    static func optionalArgument(_ value: String?) -> String? {
        guard let trimmed = value?.trimmingCharacters(in: .whitespacesAndNewlines), !trimmed.isEmpty else {
            return nil
        }
        return stripLeadingDashes(trimmed)
    }

    static func root(_ value: String?) -> String {
        let trimmed = value?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return trimmed.isEmpty ? "~" : trimmed
    }

    static func identityFile(_ value: String?) -> String? {
        let trimmed = value?.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.flatMap { $0.isEmpty ? nil : $0 }
    }

    private static func stripLeadingDashes(_ value: String) -> String {
        value.hasPrefix("-") ? String(value.drop { $0 == "-" }) : value
    }
}

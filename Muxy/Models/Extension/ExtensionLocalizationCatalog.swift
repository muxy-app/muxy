import Foundation

enum ExtensionLocalizationCatalog {
    static let pluralFormatKey = "NSStringLocalizedFormatKey"
    static let pluralSpecTypeKey = "NSStringFormatSpecTypeKey"
    static let pluralValueTypeKey = "NSStringFormatValueTypeKey"

    private static let maxPluralNestingDepth = 8

    static func incompatibleKey(in catalog: [String: Any]) -> String? {
        for key in catalog.keys.sorted() {
            guard isCompatible(value: catalog[key], with: key) else { return key }
        }
        return nil
    }

    private static func isCompatible(value: Any?, with key: String) -> Bool {
        switch value {
        case let translation as String:
            guard needsValidation(key: key, translation: translation) else { return true }
            guard let keyFormat = FormatSignature(key),
                  let translationFormat = FormatSignature(translation)
            else { return false }
            return translationFormat.satisfies(keyFormat)
        case let pluralEntry as [String: Any]:
            guard let keyFormat = FormatSignature(key) else { return false }
            return isPluralEntryCompatible(pluralEntry, with: keyFormat)
        default:
            return false
        }
    }

    private static func needsValidation(key: String, translation: String) -> Bool {
        key.contains("%") || translation.contains("%")
    }

    private static func isPluralEntryCompatible(
        _ entry: [String: Any],
        with keyFormat: FormatSignature
    ) -> Bool {
        guard let format = entry[pluralFormatKey] as? String else { return false }
        return isPluralFormatCompatible(
            format,
            firstArgumentPosition: 1,
            entry: entry,
            keyFormat: keyFormat,
            depth: 0
        )
    }

    private static func isPluralFormatCompatible(
        _ format: String,
        firstArgumentPosition: Int,
        entry: [String: Any],
        keyFormat: FormatSignature,
        depth: Int
    ) -> Bool {
        guard depth <= maxPluralNestingDepth,
              let signature = FormatSignature(format, firstArgumentPosition: firstArgumentPosition),
              signature.satisfies(keyFormat)
        else { return false }

        for variable in signature.variables {
            guard let rule = entry[variable.name] as? [String: Any] else { return false }
            for (ruleKey, ruleValue) in rule {
                if ruleKey == pluralSpecTypeKey || ruleKey == pluralValueTypeKey {
                    guard ruleValue is String else { return false }
                    continue
                }
                guard let variant = ruleValue as? String,
                      isPluralFormatCompatible(
                          variant,
                          firstArgumentPosition: variable.position,
                          entry: entry,
                          keyFormat: keyFormat,
                          depth: depth + 1
                      )
                else { return false }
            }
        }
        return true
    }
}

struct FormatSignature {
    struct VariableReference: Equatable {
        let name: String
        let position: Int
    }

    private enum ArgumentType: Equatable {
        case object
        case cString
        case unicharString
        case pointer
        case unichar
        case int32
        case uint32
        case int64
        case uint64
        case double
        case longDouble
    }

    private static let flagScalars = Set("-+ #0'".unicodeScalars)
    private static let lengthScalars = Set("hlLqjzt".unicodeScalars)

    private var typesByPosition: [Int: ArgumentType] = [:]
    private(set) var variables: [VariableReference] = []

    init?(_ format: String, firstArgumentPosition: Int = 1) {
        guard firstArgumentPosition >= 1 else { return nil }
        let scalars = Array(format.unicodeScalars)
        var nextPosition = firstArgumentPosition
        var index = 0

        while index < scalars.count {
            guard scalars[index] == "%" else {
                index += 1
                continue
            }
            index += 1
            guard index < scalars.count else { return nil }
            if scalars[index] == "%" {
                index += 1
                continue
            }
            if scalars[index] == "#" {
                guard let variable = Self.readVariable(scalars, from: index, position: nextPosition) else {
                    return nil
                }
                variables.append(variable.reference)
                nextPosition = max(nextPosition, variable.reference.position + 1)
                index = variable.end
                continue
            }
            guard let specifier = Self.readSpecifier(scalars, from: index, implicitPosition: nextPosition) else {
                return nil
            }
            if let existing = typesByPosition[specifier.position], existing != specifier.type {
                return nil
            }
            typesByPosition[specifier.position] = specifier.type
            nextPosition = max(nextPosition, specifier.position + 1)
            index = specifier.end
        }
    }

    func satisfies(_ other: FormatSignature) -> Bool {
        typesByPosition.allSatisfy { other.typesByPosition[$0.key] == $0.value }
    }

    private static func readVariable(
        _ scalars: [Unicode.Scalar],
        from start: Int,
        position: Int
    ) -> (reference: VariableReference, end: Int)? {
        var index = start + 1
        guard index < scalars.count, scalars[index] == "@" else { return nil }
        index += 1
        var name = ""
        while index < scalars.count, scalars[index] != "@" {
            name.unicodeScalars.append(scalars[index])
            index += 1
        }
        guard index < scalars.count, !name.isEmpty else { return nil }
        return (VariableReference(name: name, position: position), index + 1)
    }

    private struct ParsedSpecifier {
        let position: Int
        let type: ArgumentType
        let end: Int
    }

    private static func readSpecifier(
        _ scalars: [Unicode.Scalar],
        from start: Int,
        implicitPosition: Int
    ) -> ParsedSpecifier? {
        var index = start
        var explicitPosition: Int?
        var digits = ""
        var cursor = index
        while cursor < scalars.count, isASCIIDigit(scalars[cursor]) {
            digits.unicodeScalars.append(scalars[cursor])
            cursor += 1
        }
        if !digits.isEmpty, cursor < scalars.count, scalars[cursor] == "$" {
            guard let parsed = Int(digits), parsed >= 1 else { return nil }
            explicitPosition = parsed
            index = cursor + 1
        }

        while index < scalars.count, flagScalars.contains(scalars[index]) {
            index += 1
        }
        guard let afterWidth = skipFieldSize(scalars, from: index) else { return nil }
        index = afterWidth
        if index < scalars.count, scalars[index] == "." {
            guard let afterPrecision = skipFieldSize(scalars, from: index + 1) else { return nil }
            index = afterPrecision
        }

        var length = ""
        while index < scalars.count, lengthScalars.contains(scalars[index]) {
            length.unicodeScalars.append(scalars[index])
            index += 1
        }
        guard index < scalars.count,
              let type = argumentType(length: length, conversion: scalars[index])
        else { return nil }
        return ParsedSpecifier(
            position: explicitPosition ?? implicitPosition,
            type: type,
            end: index + 1
        )
    }

    private static func skipFieldSize(_ scalars: [Unicode.Scalar], from start: Int) -> Int? {
        guard start < scalars.count else { return start }
        guard scalars[start] != "*" else { return nil }
        var index = start
        while index < scalars.count, isASCIIDigit(scalars[index]) {
            index += 1
        }
        return index
    }

    private static func isASCIIDigit(_ scalar: Unicode.Scalar) -> Bool {
        (48 ... 57).contains(scalar.value)
    }

    private static func argumentType(length: String, conversion: Unicode.Scalar) -> ArgumentType? {
        switch conversion {
        case "@":
            length.isEmpty ? .object : nil
        case "d",
             "i":
            signedType(length: length)
        case "u",
             "x",
             "X",
             "o":
            unsignedType(length: length)
        case "f",
             "F",
             "e",
             "E",
             "g",
             "G",
             "a",
             "A":
            switch length {
            case "": .double
            case "L": .longDouble
            default: nil
            }
        case "c":
            length.isEmpty ? .int32 : nil
        case "C":
            length.isEmpty ? .unichar : nil
        case "s":
            length.isEmpty ? .cString : nil
        case "S":
            length.isEmpty ? .unicharString : nil
        case "p":
            length.isEmpty ? .pointer : nil
        default:
            nil
        }
    }

    private static func signedType(length: String) -> ArgumentType? {
        switch length {
        case "",
             "h",
             "hh": .int32
        case "l",
             "ll",
             "q",
             "j",
             "z",
             "t": .int64
        default: nil
        }
    }

    private static func unsignedType(length: String) -> ArgumentType? {
        switch length {
        case "",
             "h",
             "hh": .uint32
        case "l",
             "ll",
             "q",
             "j",
             "z",
             "t": .uint64
        default: nil
        }
    }
}

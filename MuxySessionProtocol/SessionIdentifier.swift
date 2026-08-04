public struct SessionIdentifier: Hashable, Sendable {
    public static let byteCount = 16

    public let bytes: [UInt8]

    public init?(bytes: [UInt8]) {
        guard bytes.count == Self.byteCount else { return nil }
        self.bytes = bytes
    }

    public init?(uuidString: String) {
        var parsed: [UInt8] = []
        parsed.reserveCapacity(Self.byteCount)
        var pendingHighNibble: UInt8?
        for character in uuidString {
            if character == "-" {
                continue
            }
            guard let nibble = Self.nibble(character) else { return nil }
            guard let high = pendingHighNibble else {
                pendingHighNibble = nibble
                continue
            }
            guard parsed.count < Self.byteCount else { return nil }
            parsed.append(high << 4 | nibble)
            pendingHighNibble = nil
        }
        guard pendingHighNibble == nil, parsed.count == Self.byteCount else { return nil }
        bytes = parsed
    }

    public var uuidString: String {
        var result = ""
        result.reserveCapacity(36)
        for (index, byte) in bytes.enumerated() {
            if index == 4 || index == 6 || index == 8 || index == 10 {
                result.append("-")
            }
            result.append(Self.hexDigit(byte >> 4))
            result.append(Self.hexDigit(byte & 0x0F))
        }
        return result
    }

    private static func nibble(_ character: Character) -> UInt8? {
        switch character {
        case "0" ... "9":
            UInt8(character.asciiValue ?? 0) - UInt8(ascii: "0")
        case "a" ... "f":
            UInt8(character.asciiValue ?? 0) - UInt8(ascii: "a") + 10
        case "A" ... "F":
            UInt8(character.asciiValue ?? 0) - UInt8(ascii: "A") + 10
        default:
            nil
        }
    }

    private static func hexDigit(_ value: UInt8) -> Character {
        let digits: [Character] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f"]
        return digits[Int(value & 0x0F)]
    }
}

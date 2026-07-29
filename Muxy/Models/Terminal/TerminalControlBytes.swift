import Foundation

enum TerminalControlBytes {
    static let carriageReturn = Data([0x0D])
    static let killLineToCursor = Data([0x15])
    static let backspace = Data([0x7F])
    static let pasteShortcut = Data([0x16])
    static let bracketedPasteStart = Data([0x1B, 0x5B, 0x32, 0x30, 0x30, 0x7E])
    static let bracketedPasteEnd = Data([0x1B, 0x5B, 0x32, 0x30, 0x31, 0x7E])

    static func killInput(lineBreakCount: Int) -> Data {
        var payload = killLineToCursor
        for _ in 0 ..< max(0, lineBreakCount) {
            payload += backspace
            payload += killLineToCursor
        }
        return payload
    }
}

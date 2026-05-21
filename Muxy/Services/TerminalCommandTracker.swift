import Foundation
import GhosttyKit

@MainActor
final class TerminalCommandTracker {
    static let shared = TerminalCommandTracker()

    private var buffers: [UUID: String] = [:]
    private var unreliableBuffers: Set<UUID> = []
    private var pendingCommands: [UUID: String] = [:]
    private var confirmedCommands: [UUID: String] = [:]
    private var secureInputPanes: Set<UUID> = []

    private init() {}

    func recordText(_ text: String, paneID: UUID) {
        for character in text {
            switch character {
            case "\n",
                 "\r":
                submitBuffer(paneID: paneID)
            case "\u{7F}",
                 "\u{8}":
                removeLastCharacter(paneID: paneID)
            default:
                guard character.isShellCommandText else {
                    markBufferUnreliable(paneID: paneID)
                    continue
                }
                buffers[paneID, default: ""].append(character)
            }
        }
    }

    func recordReturn(paneID: UUID) {
        submitBuffer(paneID: paneID)
    }

    func recordBackspace(paneID: UUID) {
        removeLastCharacter(paneID: paneID)
    }

    func recordShellCommandCandidate(_ command: String, paneID: UUID) {
        let trimmed = command.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let pending = pendingCommands[paneID] else { return }
        guard Self.commandsAreCompatible(pending: pending, candidate: trimmed) else { return }
        recordShellCommand(trimmed, paneID: paneID)
    }

    func confirmCommand(paneID: UUID) {
        guard let pending = pendingCommands[paneID] else { return }
        confirmedCommands[paneID] = pending
        pendingCommands.removeValue(forKey: paneID)
    }

    func setSecureInput(_ action: ghostty_action_secure_input_e, paneID: UUID) {
        switch action {
        case GHOSTTY_SECURE_INPUT_ON:
            secureInputPanes.insert(paneID)
        case GHOSTTY_SECURE_INPUT_OFF:
            secureInputPanes.remove(paneID)
        case GHOSTTY_SECURE_INPUT_TOGGLE:
            if secureInputPanes.contains(paneID) {
                secureInputPanes.remove(paneID)
            } else {
                secureInputPanes.insert(paneID)
            }
        default:
            break
        }
    }

    func lastSubmittedCommand(for paneID: UUID) -> String? {
        pendingCommands[paneID] ?? confirmedCommands[paneID]
    }

    func clearBuffer(paneID: UUID) {
        buffers[paneID] = ""
        unreliableBuffers.remove(paneID)
    }

    func removePane(_ paneID: UUID) {
        buffers.removeValue(forKey: paneID)
        unreliableBuffers.remove(paneID)
        pendingCommands.removeValue(forKey: paneID)
        confirmedCommands.removeValue(forKey: paneID)
        secureInputPanes.remove(paneID)
    }

    private func submitBuffer(paneID: UUID) {
        guard !secureInputPanes.contains(paneID) else {
            buffers[paneID] = ""
            unreliableBuffers.remove(paneID)
            return
        }
        guard !unreliableBuffers.contains(paneID) else {
            buffers[paneID] = ""
            unreliableBuffers.remove(paneID)
            return
        }
        let command = buffers[paneID, default: ""].trimmingCharacters(in: .whitespacesAndNewlines)
        buffers[paneID] = ""
        guard !command.isEmpty else { return }
        pendingCommands[paneID] = command
    }

    private func markBufferUnreliable(paneID: UUID) {
        unreliableBuffers.insert(paneID)
    }

    private func recordShellCommand(_ command: String, paneID: UUID) {
        buffers[paneID] = ""
        unreliableBuffers.remove(paneID)
        pendingCommands[paneID] = command
    }

    private func removeLastCharacter(paneID: UUID) {
        guard !(buffers[paneID]?.isEmpty ?? true) else { return }
        buffers[paneID]?.removeLast()
    }

    private static func commandsAreCompatible(pending: String, candidate: String) -> Bool {
        guard !candidate.isEmpty else { return false }
        guard firstWord(in: pending) == firstWord(in: candidate) else { return false }
        return candidate.count >= pending.count
    }

    private static func firstWord(in command: String) -> String {
        command
            .split(whereSeparator: \.isWhitespace)
            .first
            .map(String.init) ?? ""
    }
}

private extension Character {
    var isShellCommandText: Bool {
        unicodeScalars.allSatisfy { scalar in
            scalar.value >= 0x20 && scalar.value != 0x7F
        }
    }
}

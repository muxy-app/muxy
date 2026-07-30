import Foundation

final class TerminalShellActivityTracker: @unchecked Sendable {
    final class Session: @unchecked Sendable {
        private let tracker: TerminalShellActivityTracker
        private let generation: UInt64

        fileprivate init(tracker: TerminalShellActivityTracker, generation: UInt64) {
            self.tracker = tracker
            self.generation = generation
        }

        func recordOutput(_ data: Data) {
            tracker.recordOutput(data, generation: generation)
        }

        func invalidate() {
            tracker.invalidate(generation: generation)
        }
    }

    private let lock = NSLock()
    private var parser = SemanticCommandParser()
    private var commandRunning = false
    private var generation: UInt64 = 0

    var isCommandRunning: Bool {
        lock.withLock { commandRunning }
    }

    func beginSession() -> Session {
        lock.withLock {
            generation &+= 1
            parser.reset()
            commandRunning = false
            return Session(tracker: self, generation: generation)
        }
    }

    private func recordOutput(_ data: Data, generation: UInt64) {
        lock.withLock {
            guard generation == self.generation else { return }
            for byte in data {
                guard let event = parser.event(for: byte) else { continue }
                switch event {
                case .started:
                    commandRunning = true
                case .finished:
                    commandRunning = false
                }
            }
        }
    }

    private func invalidate(generation: UInt64) {
        lock.withLock {
            guard generation == self.generation else { return }
            self.generation &+= 1
            parser.reset()
            commandRunning = false
        }
    }
}

private struct SemanticCommandParser {
    enum Event {
        case started
        case finished
    }

    private enum State {
        case idle
        case escape
        case osc
        case oscOne
        case oscThirteen
        case oscOneThirtyThree
        case action
        case payload(Event)
        case payloadEscape(Event)
    }

    private var state = State.idle

    mutating func event(for byte: UInt8) -> Event? {
        switch state {
        case .idle:
            state = byte == 0x1B ? .escape : .idle
        case .escape:
            if byte == 0x5D {
                state = .osc
            } else {
                state = byte == 0x1B ? .escape : .idle
            }
        case .osc:
            state = nextState(byte: byte, expected: 0x31, matched: .oscOne)
        case .oscOne:
            state = nextState(byte: byte, expected: 0x33, matched: .oscThirteen)
        case .oscThirteen:
            state = nextState(byte: byte, expected: 0x33, matched: .oscOneThirtyThree)
        case .oscOneThirtyThree:
            state = nextState(byte: byte, expected: 0x3B, matched: .action)
        case .action:
            switch byte {
            case 0x43:
                state = .payload(.started)
            case 0x44:
                state = .payload(.finished)
            default:
                state = byte == 0x1B ? .escape : .idle
            }
        case let .payload(event):
            if byte == 0x07 {
                state = .idle
                return event
            }
            state = byte == 0x1B ? .payloadEscape(event) : .payload(event)
        case let .payloadEscape(event):
            if byte == 0x5C {
                state = .idle
                return event
            }
            if byte == 0x5D {
                state = .osc
            } else {
                state = byte == 0x1B ? .payloadEscape(event) : .payload(event)
            }
        }
        return nil
    }

    mutating func reset() {
        state = .idle
    }

    private func nextState(byte: UInt8, expected: UInt8, matched: State) -> State {
        if byte == expected {
            return matched
        }
        return byte == 0x1B ? .escape : .idle
    }
}

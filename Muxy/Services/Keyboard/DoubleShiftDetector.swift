import Foundation

struct DoubleShiftModifierState {
    private var capsLockEnabled: Bool?

    mutating func otherModifierPressed(
        conventionalModifierPressed: Bool,
        capsLockEnabled currentCapsLockState: Bool
    ) -> Bool {
        let capsLockChanged = capsLockEnabled.map { $0 != currentCapsLockState } ?? false
        capsLockEnabled = currentCapsLockState
        return conventionalModifierPressed || capsLockChanged
    }

    mutating func reset() {
        capsLockEnabled = nil
    }
}

struct DoubleShiftDetector {
    struct Configuration: Equatable {
        var maximumTapDuration: TimeInterval
        var maximumInterval: TimeInterval

        init(maximumTapDuration: TimeInterval = 0.35, maximumInterval: TimeInterval = 0.5) {
            self.maximumTapDuration = maximumTapDuration
            self.maximumInterval = maximumInterval
        }
    }

    enum Input: Equatable {
        case modifierChange(shiftPressed: Bool, otherModifierPressed: Bool, timestamp: TimeInterval)
        case keyDown(shiftPressed: Bool, timestamp: TimeInterval)
        case pointerDown(shiftPressed: Bool, timestamp: TimeInterval)

        var timestamp: TimeInterval {
            switch self {
            case let .modifierChange(_, _, timestamp),
                 let .keyDown(_, timestamp),
                 let .pointerDown(_, timestamp): timestamp
            }
        }

        var shiftPressed: Bool {
            switch self {
            case let .modifierChange(shiftPressed, _, _),
                 let .keyDown(shiftPressed, _),
                 let .pointerDown(shiftPressed, _): shiftPressed
            }
        }
    }

    private enum State: Equatable {
        case idle
        case firstPress(TimeInterval)
        case awaitingSecondPress(TimeInterval)
        case secondPress(TimeInterval)
        case blockedUntilShiftRelease
    }

    private let configuration: Configuration
    private var state = State.idle
    private var lastTimestamp: TimeInterval?

    init(configuration: Configuration = Configuration()) {
        self.configuration = configuration
    }

    mutating func process(_ input: Input) -> Bool {
        if let lastTimestamp, input.timestamp < lastTimestamp {
            state = .idle
        }
        lastTimestamp = input.timestamp

        switch input {
        case let .keyDown(shiftPressed, _),
             let .pointerDown(shiftPressed, _):
            state = shiftPressed ? .blockedUntilShiftRelease : .idle
            return false
        case let .modifierChange(shiftPressed, otherModifierPressed, timestamp):
            return processModifierChange(
                shiftPressed: shiftPressed,
                otherModifierPressed: otherModifierPressed,
                timestamp: timestamp
            )
        }
    }

    mutating func reset() {
        state = .idle
        lastTimestamp = nil
    }

    private mutating func processModifierChange(
        shiftPressed: Bool,
        otherModifierPressed: Bool,
        timestamp: TimeInterval
    ) -> Bool {
        if otherModifierPressed {
            state = shiftPressed ? .blockedUntilShiftRelease : .idle
            return false
        }

        if case .blockedUntilShiftRelease = state {
            if !shiftPressed {
                state = .idle
            }
            return false
        }

        expireState(at: timestamp, shiftPressed: shiftPressed)

        if shiftPressed {
            switch state {
            case .idle:
                state = .firstPress(timestamp)
            case .awaitingSecondPress:
                state = .secondPress(timestamp)
            case .firstPress,
                 .secondPress,
                 .blockedUntilShiftRelease:
                break
            }
            return false
        }

        switch state {
        case let .firstPress(pressedAt):
            guard timestamp - pressedAt <= configuration.maximumTapDuration else {
                state = .idle
                return false
            }
            state = .awaitingSecondPress(timestamp)
            return false
        case let .secondPress(pressedAt):
            state = .idle
            return timestamp - pressedAt <= configuration.maximumTapDuration
        case .idle,
             .awaitingSecondPress,
             .blockedUntilShiftRelease:
            return false
        }
    }

    private mutating func expireState(at timestamp: TimeInterval, shiftPressed: Bool) {
        switch state {
        case let .firstPress(pressedAt),
             let .secondPress(pressedAt):
            guard timestamp - pressedAt > configuration.maximumTapDuration else { return }
            state = shiftPressed ? .blockedUntilShiftRelease : .idle
        case let .awaitingSecondPress(releasedAt):
            guard timestamp - releasedAt > configuration.maximumInterval else { return }
            state = .idle
        case .idle,
             .blockedUntilShiftRelease:
            break
        }
    }
}

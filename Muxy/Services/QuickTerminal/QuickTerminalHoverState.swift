struct QuickTerminalHoverState: Equatable {
    enum Mode: Equatable {
        case armed
        case dwelling
        case open
        case disarmed
    }

    enum Input: Equatable {
        case pointerEntered
        case pointerExited
        case dwellElapsed
        case terminalOpened
        case terminalClosed(pointerInside: Bool)
    }

    enum Effect: Equatable {
        case none
        case startDwellTimer
        case cancelDwellTimer
        case requestOpen
    }

    private(set) var mode: Mode = .armed
    private(set) var pointerInside = false

    mutating func handle(_ input: Input) -> Effect {
        switch input {
        case .pointerEntered:
            pointerInside = true
            guard mode == .armed else { return .none }
            mode = .dwelling
            return .startDwellTimer
        case .pointerExited:
            pointerInside = false
            switch mode {
            case .dwelling:
                mode = .armed
                return .cancelDwellTimer
            case .disarmed:
                mode = .armed
                return .none
            case .armed,
                 .open:
                return .none
            }
        case .dwellElapsed:
            guard mode == .dwelling else { return .none }
            mode = .open
            return .requestOpen
        case .terminalOpened:
            mode = .open
            return .cancelDwellTimer
        case let .terminalClosed(pointerInside):
            reset(pointerInside: pointerInside)
            return .none
        }
    }

    mutating func reset(pointerInside: Bool) {
        self.pointerInside = pointerInside
        mode = pointerInside ? .disarmed : .armed
    }
}

struct AgentHookCommand: Equatable {
    let provider: String
    let providerTitle: String
    let event: String

    static func parse(_ arguments: [String]) -> AgentHookCommand? {
        guard arguments.first == "agent-event" else { return nil }

        var provider: String?
        var providerTitle: String?
        var event: String?
        var index = 1

        while index < arguments.count {
            guard index + 1 < arguments.count else { return nil }
            let value = arguments[index + 1]
            switch arguments[index] {
            case "--provider":
                provider = value
            case "--provider-title":
                providerTitle = value
            case "--event":
                event = value
            default:
                return nil
            }
            index += 2
        }

        guard let provider, !provider.isEmpty,
              let providerTitle,
              let event, !event.isEmpty
        else { return nil }

        return AgentHookCommand(provider: provider, providerTitle: providerTitle, event: event)
    }
}

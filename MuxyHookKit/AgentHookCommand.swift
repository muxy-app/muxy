public struct AgentHookCommand: Equatable {
    public let provider: String
    public let providerTitle: String
    public let event: String
    public let test: Bool

    public init(provider: String, providerTitle: String, event: String, test: Bool = false) {
        self.provider = provider
        self.providerTitle = providerTitle
        self.event = event
        self.test = test
    }

    public static func parse(_ arguments: [String]) -> AgentHookCommand? {
        guard arguments.first == "agent-event" else { return nil }

        var provider: String?
        var providerTitle: String?
        var event: String?
        var test = false
        var index = 1

        while index < arguments.count {
            if arguments[index] == "--test" {
                test = true
                index += 1
                continue
            }
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

        return AgentHookCommand(provider: provider, providerTitle: providerTitle, event: event, test: test)
    }
}

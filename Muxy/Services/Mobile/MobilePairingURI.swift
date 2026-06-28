import Foundation

enum MobilePairingURI {
    static let scheme = "muxy"
    static let action = "pair"

    static func makeString(
        host: String,
        port: UInt16,
        service: String?,
        label: String?
    ) -> String? {
        let trimmedHost = host.trimmingCharacters(in: .whitespaces)
        guard !trimmedHost.isEmpty else { return nil }

        var components = URLComponents()
        components.scheme = scheme
        components.host = action

        var items: [URLQueryItem] = [
            URLQueryItem(name: "host", value: trimmedHost),
            URLQueryItem(name: "port", value: String(port)),
        ]
        if let service, !service.isEmpty {
            items.append(URLQueryItem(name: "service", value: service))
        }
        if let label, !label.isEmpty {
            items.append(URLQueryItem(name: "label", value: label))
        }
        components.queryItems = items

        return components.url?.absoluteString
    }
}

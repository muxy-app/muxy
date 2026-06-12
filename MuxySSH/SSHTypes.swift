import Foundation

public protocol SSHConnectionConfigurable: Sendable {
    var host: String { get }
    var port: Int { get }
    var user: String { get }
    var remoteExecCommand: String? { get }
    var initialShellInput: String { get }
    var authentication: SSHAuthentication? { get }
}

public enum SSHAuthentication: Equatable, Sendable {
    case privateKey(path: String)
    case password(String)
}

public enum SSHConnectionError: Error {
    case refused(String)
    case authFailed(String)
    case hostKeyChanged(String)
    case unknownHostKey(String)
    case timeout(String)
    case unknown(String)

    public var title: String {
        switch self {
        case .refused: "Connection Refused"
        case .authFailed: "Authentication Failed"
        case .hostKeyChanged: "Host Key Changed"
        case .unknownHostKey: "Unknown Host Key"
        case .timeout: "Connection Timeout"
        case .unknown: "Connection Error"
        }
    }

    public var message: String {
        switch self {
        case let .refused(host): "Could not connect to \(host): Connection refused"
        case let .authFailed(detail): detail
        case let .hostKeyChanged(detail): detail
        case let .unknownHostKey(host): "The host key for \(host) is not in known_hosts. Add it to ~/.ssh/known_hosts to connect."
        case let .timeout(host): "Connection to \(host) timed out"
        case let .unknown(detail): detail
        }
    }
}

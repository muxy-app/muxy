import Foundation

struct RemoteTmuxSession: Hashable {
    let id: UUID
    let destination: SSHDestination

    init(id: UUID = UUID(), destination: SSHDestination) {
        self.id = id
        self.destination = destination
    }

    var name: String {
        "muxy-" + id.uuidString.lowercased().replacingOccurrences(of: "-", with: "")
    }

    var target: String { "=" + name }
}

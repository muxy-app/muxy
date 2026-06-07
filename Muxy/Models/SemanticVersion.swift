import Foundation

struct SemanticVersion: Comparable, Equatable {
    let components: [Int]

    init?(_ string: String) {
        let core = string.split(separator: "+", maxSplits: 1).first ?? Substring(string)
        let release = core.split(separator: "-", maxSplits: 1).first ?? core
        let parsed = release.split(separator: ".").map { Int($0) }
        guard !parsed.isEmpty, parsed.allSatisfy({ $0 != nil }) else { return nil }
        components = parsed.compactMap(\.self)
    }

    static func < (lhs: SemanticVersion, rhs: SemanticVersion) -> Bool {
        let count = max(lhs.components.count, rhs.components.count)
        for index in 0 ..< count where lhs.component(at: index) != rhs.component(at: index) {
            return lhs.component(at: index) < rhs.component(at: index)
        }
        return false
    }

    static func == (lhs: SemanticVersion, rhs: SemanticVersion) -> Bool {
        let count = max(lhs.components.count, rhs.components.count)
        return (0 ..< count).allSatisfy { lhs.component(at: $0) == rhs.component(at: $0) }
    }

    private func component(at index: Int) -> Int {
        index < components.count ? components[index] : 0
    }

    static func isUpdate(installed: String, available: String) -> Bool {
        guard let installed = SemanticVersion(installed),
              let available = SemanticVersion(available)
        else { return false }
        return available > installed
    }
}

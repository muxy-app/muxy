import Foundation

struct RepositoryChangesFileSelection: Equatable {
    enum Click: Equatable {
        case exclusive
        case toggle
        case range
        case addingRange

        static func from(command: Bool, shift: Bool) -> Click {
            switch (command, shift) {
            case (true, true):
                .addingRange
            case (false, true):
                .range
            case (true, false):
                .toggle
            case (false, false):
                .exclusive
            }
        }
    }

    private(set) var selectedIDs: Set<String> = []
    private(set) var anchorID: String?

    var isEmpty: Bool { selectedIDs.isEmpty }

    func contains(_ id: String) -> Bool {
        selectedIDs.contains(id)
    }

    func files(in files: [GitStatusFile]) -> [GitStatusFile] {
        files.filter { selectedIDs.contains($0.id) }
    }

    func actionTargets(_ file: GitStatusFile, in files: [GitStatusFile]) -> [GitStatusFile] {
        let selected = self.files(in: files)
        if selected.contains(where: { $0.id == file.id }) {
            return selected
        }
        return [file]
    }

    mutating func handleClick(id: String, ids: [String], kind: Click) {
        guard ids.contains(id) else { return }

        switch kind {
        case .exclusive:
            if selectedIDs == [id] {
                selectedIDs = []
                anchorID = nil
                return
            }
            selectedIDs = [id]
            anchorID = id
        case .toggle:
            if selectedIDs.contains(id) {
                selectedIDs.remove(id)
            } else {
                selectedIDs.insert(id)
            }
            anchorID = id
        case .range:
            selectedIDs = rangedIDs(to: id, ids: ids)
        case .addingRange:
            selectedIDs.formUnion(rangedIDs(to: id, ids: ids))
        }
    }

    mutating func retain(ids: [String]) {
        let valid = Set(ids)
        selectedIDs.formIntersection(valid)
        if let anchorID, !valid.contains(anchorID) {
            self.anchorID = nil
        }
    }

    mutating func remove(ids: [String]) {
        selectedIDs.subtract(ids)
        if let anchorID, !selectedIDs.contains(anchorID) {
            self.anchorID = nil
        }
    }

    private mutating func rangedIDs(to id: String, ids: [String]) -> Set<String> {
        guard let end = ids.firstIndex(of: id) else { return [] }
        let start: Int
        if let anchorID, let anchorIndex = ids.firstIndex(of: anchorID) {
            start = anchorIndex
        } else {
            start = end
            self.anchorID = id
        }
        return Set(ids[min(start, end) ... max(start, end)])
    }
}

import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "RichInputDraftStore")

@MainActor
@Observable
final class RichInputDraftStore {
    static let shared = RichInputDraftStore()

    @ObservationIgnored private var drafts: [String: RichInputDraft] = [:]
    @ObservationIgnored private let store: CodableFileStore<[String: RichInputDraft]>
    @ObservationIgnored private var pendingSave: Task<Void, Never>?
    @ObservationIgnored private static let saveDebounce: Duration = .milliseconds(400)

    init(
        fileURL: URL = MuxyFileStorage.fileURL(filename: "rich-input-drafts.json")
    ) {
        store = CodableFileStore(
            fileURL: fileURL,
            options: CodableFileStoreOptions(
                prettyPrinted: true,
                sortedKeys: true,
                filePermissions: FilePermissions.privateFile
            )
        )
        load()
        sweepOrphanImages()
    }

    func draft(for key: WorktreeKey) -> RichInputDraft? {
        drafts[Self.identifier(for: key)]
    }

    func scheduleSave(_ draft: RichInputDraft, for key: WorktreeKey) {
        let id = Self.identifier(for: key)
        if draft.isEmpty {
            guard drafts.removeValue(forKey: id) != nil else { return }
        } else {
            guard drafts[id] != draft else { return }
            drafts[id] = draft
        }
        pendingSave?.cancel()
        pendingSave = Task { @MainActor [weak self] in
            try? await Task.sleep(for: Self.saveDebounce)
            guard !Task.isCancelled else { return }
            self?.persist()
        }
    }

    func flush() {
        pendingSave?.cancel()
        pendingSave = nil
        persist()
        sweepOrphanImages()
    }

    private func persist() {
        do {
            try store.save(drafts)
        } catch {
            logger.error("Failed to save rich input drafts: \(error.localizedDescription)")
        }
    }

    private func load() {
        do {
            drafts = try store.load() ?? [:]
        } catch {
            logger.error("Failed to load rich input drafts: \(error.localizedDescription)")
            drafts = [:]
        }
    }

    private func sweepOrphanImages() {
        let referenced = Set(
            drafts.values
                .flatMap(\.imageAttachments)
                .map(\.lastPathComponent)
        )
        RichInputImageStorage.removeOrphans(referencedFilenames: referenced)
    }

    static func identifier(for key: WorktreeKey) -> String {
        "\(key.projectID.uuidString):\(key.worktreeID.uuidString)"
    }
}

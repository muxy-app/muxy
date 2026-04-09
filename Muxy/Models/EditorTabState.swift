import Foundation

enum EditorSearchNavigationDirection {
    case next
    case previous
}

@MainActor
@Observable
final class EditorTabState: Identifiable {
    let id = UUID()
    let projectPath: String
    let filePath: String
    var content: String = ""
    var isLoading = false
    var isModified = false
    var isSaving = false
    var errorMessage: String?
    var cursorLine: Int = 1
    var cursorColumn: Int = 1
    var searchVisible = false
    var searchNeedle = ""
    var searchMatchCount = 0
    var searchCurrentIndex = 0
    var searchNavigationVersion = 0
    var searchNavigationDirection: EditorSearchNavigationDirection = .next

    var fileName: String {
        URL(fileURLWithPath: filePath).lastPathComponent
    }

    var fileExtension: String {
        let url = URL(fileURLWithPath: filePath)
        let ext = url.pathExtension.lowercased()
        guard ext.isEmpty else { return ext }
        return url.lastPathComponent
    }

    var displayTitle: String {
        let name = fileName
        return isModified ? "\(name) \u{2022}" : name
    }

    @ObservationIgnored private var loadTask: Task<Void, Never>?

    init(projectPath: String, filePath: String) {
        self.projectPath = projectPath
        self.filePath = filePath
        loadFile()
    }

    deinit {
        loadTask?.cancel()
    }

    func loadFile() {
        guard !isLoading else { return }
        isLoading = true
        errorMessage = nil
        loadTask?.cancel()
        let path = filePath
        loadTask = Task { [weak self] in
            do {
                let text = try await Self.readFile(at: path)
                guard !Task.isCancelled, let self else { return }
                content = text
                isModified = false
                isLoading = false
            } catch {
                guard !Task.isCancelled, let self else { return }
                errorMessage = error.localizedDescription
                isLoading = false
            }
        }
    }

    private static func readFile(at path: String) async throws -> String {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    let data = try Data(contentsOf: URL(fileURLWithPath: path))
                    guard let text = String(bytes: data, encoding: .utf8) else {
                        continuation.resume(throwing: CocoaError(.fileReadUnknownStringEncoding))
                        return
                    }
                    continuation.resume(returning: text)
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    func saveFile() {
        guard !isSaving else { return }
        let textToSave = content
        let path = filePath
        isSaving = true
        Task { [weak self] in
            do {
                try await Self.writeFile(text: textToSave, path: path)
                guard !Task.isCancelled, let self else { return }
                isSaving = false
                isModified = false
            } catch {
                guard !Task.isCancelled, let self else { return }
                isSaving = false
                errorMessage = error.localizedDescription
            }
        }
    }

    private static func writeFile(text: String, path: String) async throws {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    try text.write(toFile: path, atomically: true, encoding: .utf8)
                    continuation.resume()
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    func markModified() {
        guard !isModified else { return }
        isModified = true
    }

    func navigateSearch(_ direction: EditorSearchNavigationDirection) {
        searchNavigationDirection = direction
        searchNavigationVersion += 1
    }
}

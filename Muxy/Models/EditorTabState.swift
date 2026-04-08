import Foundation

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

    var fileName: String {
        URL(fileURLWithPath: filePath).lastPathComponent
    }

    var fileExtension: String {
        URL(fileURLWithPath: filePath).pathExtension.lowercased()
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
            let result = await Self.readFile(at: path)
            guard !Task.isCancelled, let self else { return }
            switch result {
            case let .success(text):
                content = text
                isModified = false
            case let .failure(message):
                errorMessage = message
            }
            isLoading = false
        }
    }

    private enum FileReadResult {
        case success(String)
        case failure(String)
    }

    private static func readFile(at path: String) async -> FileReadResult {
        await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    let data = try Data(contentsOf: URL(fileURLWithPath: path))
                    guard let text = String(bytes: data, encoding: .utf8) else {
                        continuation.resume(returning: .failure("File is not valid UTF-8"))
                        return
                    }
                    continuation.resume(returning: .success(text))
                } catch {
                    continuation.resume(returning: .failure(error.localizedDescription))
                }
            }
        }
    }

    func saveFile() {
        let textToSave = content
        guard !isSaving else { return }
        isSaving = true
        Task { [weak self] in
            guard let self else { return }
            defer { isSaving = false }
            do {
                try textToSave.write(toFile: filePath, atomically: true, encoding: .utf8)
                guard !Task.isCancelled else { return }
                isModified = false
            } catch {
                guard !Task.isCancelled else { return }
                errorMessage = error.localizedDescription
            }
        }
    }

    func markModified() {
        guard !isModified else { return }
        isModified = true
    }
}

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
    var isReadOnly = false
    var cursorLine: Int = 1
    var cursorColumn: Int = 1
    var searchVisible = false
    var searchFocusVersion = 0
    var searchNeedle = ""
    var searchMatchCount = 0
    var searchCurrentIndex = 0
    var searchNavigationVersion = 0
    var searchNavigationDirection: EditorSearchNavigationDirection = .next
    var searchCaseSensitive = false
    var searchUseRegex = false
    var searchInvalidRegex = false
    var replaceVisible = false
    var replaceText = ""
    var replaceVersion = 0
    var replaceAllVersion = 0
    var currentSelection = ""
    var awaitingLargeFileConfirmation = false
    var largeFileSize: Int64 = 0

    static let largeFileWarningThreshold: Int64 = 5 * 1024 * 1024
    static let largeFileRefuseThreshold: Int64 = 50 * 1024 * 1024

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

    private enum SaveError: LocalizedError {
        case fileIsReadOnly(String)

        var errorDescription: String? {
            switch self {
            case let .fileIsReadOnly(path):
                "File is read-only: \(URL(fileURLWithPath: path).lastPathComponent)"
            }
        }
    }

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
        errorMessage = nil
        refreshReadOnlyStatus()

        let size = fileSize(at: filePath)
        if size >= Self.largeFileRefuseThreshold {
            errorMessage = "File is too large to open (\(Self.formatBytes(size))). " +
                "Use a dedicated editor for files over \(Self.formatBytes(Self.largeFileRefuseThreshold))."
            isLoading = false
            return
        }
        if size >= Self.largeFileWarningThreshold {
            largeFileSize = size
            awaitingLargeFileConfirmation = true
            isLoading = false
            return
        }

        performLoad()
    }

    func confirmLargeFileOpen() {
        awaitingLargeFileConfirmation = false
        performLoad()
    }

    func cancelLargeFileOpen() {
        awaitingLargeFileConfirmation = false
        errorMessage = "File load cancelled."
    }

    private func performLoad() {
        isLoading = true
        errorMessage = nil
        loadTask?.cancel()
        let path = filePath
        loadTask = Task { [weak self] in
            do {
                let text = try await Self.readFile(at: path)
                guard !Task.isCancelled, let self else { return }
                content = text
                refreshReadOnlyStatus()
                isModified = false
                isLoading = false
            } catch {
                guard !Task.isCancelled, let self else { return }
                errorMessage = error.localizedDescription
                isLoading = false
            }
        }
    }

    private func fileSize(at path: String) -> Int64 {
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: path),
              let size = attrs[.size] as? NSNumber
        else { return 0 }
        return size.int64Value
    }

    private static func formatBytes(_ bytes: Int64) -> String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useMB, .useKB, .useGB]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: bytes)
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
        Task { [weak self] in
            try? await self?.saveFileAsync()
        }
    }

    func saveFileAsync() async throws {
        guard !isSaving else { return }
        if !content.isEmpty, !content.hasSuffix("\n") {
            content.append("\n")
        }
        let textToSave = content
        let path = filePath
        refreshReadOnlyStatus()
        guard Self.canWriteFile(at: path) else {
            throw SaveError.fileIsReadOnly(path)
        }
        isSaving = true
        do {
            try await Self.writeFile(text: textToSave, path: path)
            isSaving = false
            isModified = false
        } catch {
            isSaving = false
            throw error
        }
    }

    private static func canWriteFile(at path: String) -> Bool {
        FileManager.default.isWritableFile(atPath: path)
    }

    private func refreshReadOnlyStatus() {
        isReadOnly = !Self.canWriteFile(at: filePath)
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

    func requestReplaceCurrent() {
        replaceVersion += 1
    }

    func requestReplaceAll() {
        replaceAllVersion += 1
    }
}

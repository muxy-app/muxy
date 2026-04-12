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
    var backingStoreVersion = 0
    var isLoading = false
    var isIncrementalLoading = false
    var isModified = false
    var isSaving = false
    var errorMessage: String?
    var isReadOnly = false
    var cursorLine: Int = 1
    var cursorColumn: Int = 1
    var searchVisible = false
    var searchFocusVersion = 0
    var editorFocusVersion = 0
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
    var backingStore: TextBackingStore?

    static let largeFileWarningThreshold: Int64 = 5 * 1024 * 1024
    static let largeFileRefuseThreshold: Int64 = 50 * 1024 * 1024
    static let initialOpenChunkSize = 512 * 1024
    static let streamChunkSize = 4 * 1024 * 1024
    static let streamYieldChunkSize = 2 * 1024 * 1024

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

    private enum FileLoadEvent {
        case initial(String, hasMore: Bool)
        case appended(String)
        case finished
    }

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
        isIncrementalLoading = false
        refreshReadOnlyStatus()

        let size = fileSize(at: filePath)
        if size >= Self.largeFileRefuseThreshold {
            errorMessage = "File is too large to open (\(Self.formatBytes(size))). " +
                "Use a dedicated editor for files over \(Self.formatBytes(Self.largeFileRefuseThreshold))."
            isLoading = false
            isIncrementalLoading = false
            return
        }
        if size >= Self.largeFileWarningThreshold {
            largeFileSize = size
            awaitingLargeFileConfirmation = true
            isLoading = false
            isIncrementalLoading = false
            return
        }

        performLoad()
    }

    func confirmLargeFileOpen() {
        awaitingLargeFileConfirmation = false
        isIncrementalLoading = false
        performLoad()
    }

    func cancelLargeFileOpen() {
        awaitingLargeFileConfirmation = false
        isIncrementalLoading = false
        errorMessage = "File load cancelled."
    }

    private func performLoad() {
        isLoading = true
        isIncrementalLoading = false
        isModified = false
        errorMessage = nil
        backingStore = nil
        loadTask?.cancel()
        let path = filePath
        loadTask = Task { [weak self] in
            do {
                var hasInitialChunk = false
                for try await event in Self.streamFile(at: path) {
                    guard !Task.isCancelled, let self else { return }
                    switch event {
                    case let .initial(text, hasMore):
                        hasInitialChunk = true
                        let store = TextBackingStore()
                        store.loadFromText(text)
                        backingStore = store
                        backingStoreVersion += 1
                        refreshReadOnlyStatus()
                        isModified = false
                        isLoading = false
                        isIncrementalLoading = hasMore
                    case let .appended(text):
                        if let backingStore {
                            backingStore.appendText(text)
                            backingStoreVersion += 1
                        }
                        if isLoading {
                            isLoading = false
                        }
                        if !isIncrementalLoading {
                            isIncrementalLoading = true
                        }
                    case .finished:
                        if let backingStore {
                            backingStore.finishLoading()
                            backingStoreVersion += 1
                        }
                        refreshReadOnlyStatus()
                        if isLoading {
                            isLoading = false
                        }
                        if isIncrementalLoading {
                            isIncrementalLoading = false
                        }
                    }
                }

                guard let self else { return }
                if !hasInitialChunk {
                    isLoading = false
                    isIncrementalLoading = false
                }
            } catch {
                guard !Task.isCancelled, let self else { return }
                errorMessage = error.localizedDescription
                isLoading = false
                isIncrementalLoading = false
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

    private static func streamFile(at path: String) -> AsyncThrowingStream<FileLoadEvent, Error> {
        let initialChunkSize = initialOpenChunkSize
        let streamChunkSize = Self.streamChunkSize
        let yieldChunkSize = Self.streamYieldChunkSize
        return AsyncThrowingStream { continuation in
            let workerTask = Task.detached(priority: .userInitiated) {
                let url = URL(fileURLWithPath: path)
                do {
                    let attrs = try FileManager.default.attributesOfItem(atPath: path)
                    let fileSize = (attrs[.size] as? NSNumber)?.int64Value ?? 0
                    var pendingUTF8 = Data()

                    func decodeChunk(_ chunk: Data, isFinal: Bool) throws -> String {
                        var combined = Data()
                        combined.reserveCapacity(pendingUTF8.count + chunk.count)
                        combined.append(pendingUTF8)
                        combined.append(chunk)

                        let maxTrim = min(3, combined.count)
                        for trim in 0 ... maxTrim {
                            let end = combined.count - trim
                            let prefix = combined.prefix(end)
                            guard let text = String(bytes: prefix, encoding: .utf8) else { continue }
                            pendingUTF8 = Data(combined.suffix(trim))
                            if isFinal {
                                if pendingUTF8.isEmpty { return text }
                                guard let tail = String(bytes: pendingUTF8, encoding: .utf8) else {
                                    throw CocoaError(.fileReadUnknownStringEncoding)
                                }
                                pendingUTF8.removeAll(keepingCapacity: false)
                                return text + tail
                            }
                            return text
                        }

                        throw CocoaError(.fileReadUnknownStringEncoding)
                    }

                    let handle = try FileHandle(forReadingFrom: url)
                    defer {
                        try? handle.close()
                    }

                    let initialData = try handle.read(upToCount: initialChunkSize) ?? Data()
                    let initialText = try decodeChunk(initialData, isFinal: false)
                    let initialDataCount = Int64(initialData.count)
                    let hasMore = initialDataCount < fileSize
                    if !hasMore {
                        let tail = try decodeChunk(Data(), isFinal: true)
                        continuation.yield(FileLoadEvent.initial(initialText + tail, hasMore: false))
                        continuation.finish()
                        return
                    }

                    continuation.yield(FileLoadEvent.initial(initialText, hasMore: true))

                    var batch = ""
                    batch.reserveCapacity(yieldChunkSize)
                    var batchBytes = 0

                    while true {
                        try Task.checkCancellation()
                        let data = try handle.read(upToCount: streamChunkSize) ?? Data()
                        if data.isEmpty { break }
                        let text = try decodeChunk(data, isFinal: false)
                        if text.isEmpty { continue }
                        batch += text
                        batchBytes += data.count
                        if batchBytes >= yieldChunkSize {
                            continuation.yield(FileLoadEvent.appended(batch))
                            batch = ""
                            batchBytes = 0
                        }
                    }

                    let tail = try decodeChunk(Data(), isFinal: true)
                    if !tail.isEmpty {
                        batch += tail
                    }
                    if !batch.isEmpty {
                        continuation.yield(FileLoadEvent.appended(batch))
                    }
                    continuation.yield(FileLoadEvent.finished)
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }

            continuation.onTermination = { _ in
                workerTask.cancel()
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
        isSaving = true
        guard let store = backingStore else {
            isSaving = false
            return
        }
        let liveContent = store.fullText()
        let textToSave: String = if !liveContent.isEmpty, !liveContent.hasSuffix("\n") {
            liveContent + "\n"
        } else {
            liveContent
        }
        let path = filePath
        refreshReadOnlyStatus()
        guard Self.canWriteFile(at: path) else {
            isSaving = false
            throw SaveError.fileIsReadOnly(path)
        }
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
            DispatchQueue.global(qos: .utility).async {
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

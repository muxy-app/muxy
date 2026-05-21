import AppKit
import Foundation
import UniformTypeIdentifiers

@MainActor
@Observable
final class ImageViewerTabState: Identifiable {
    static let minScale: CGFloat = 0.05
    static let maxScale: CGFloat = 40.0
    static let zoomStep: CGFloat = 1.25

    let id = UUID()
    let projectPath: String
    private(set) var filePath: String

    var image: NSImage?
    var scale: CGFloat = 1.0
    var errorMessage: String?
    private(set) var fitTrigger: Int = 0

    @ObservationIgnored private var fileWatcher: EditorFileWatcher?
    @ObservationIgnored private var loadTask: Task<Void, Never>?

    var isLoaded: Bool { image != nil }

    var displayTitle: String {
        URL(fileURLWithPath: filePath).lastPathComponent
    }

    init(projectPath: String, filePath: String) {
        self.projectPath = projectPath
        self.filePath = filePath
        loadImage()
        installFileWatcher()
    }

    deinit {
        loadTask?.cancel()
    }

    static func canOpen(filePath: String) -> Bool {
        let ext = URL(fileURLWithPath: filePath).pathExtension.lowercased()
        guard !ext.isEmpty else { return false }
        return UTType(filenameExtension: ext)?.conforms(to: .image) == true
    }

    func requestFitToWindow() {
        fitTrigger &+= 1
    }

    func requestActualSize() {
        scale = 1.0
    }

    func zoomIn() {
        scale = min(Self.maxScale, scale * Self.zoomStep)
    }

    func zoomOut() {
        scale = max(Self.minScale, scale / Self.zoomStep)
    }

    var canZoomIn: Bool { scale < Self.maxScale }
    var canZoomOut: Bool { scale > Self.minScale }

    func updateFilePath(_ newPath: String) {
        guard filePath != newPath else { return }
        filePath = newPath
        loadImage()
        installFileWatcher()
    }

    private func loadImage() {
        loadTask?.cancel()
        errorMessage = nil
        let path = filePath
        loadTask = Task.detached(priority: .userInitiated) { [weak self] in
            let loaded = NSImage(contentsOfFile: path)
            guard !Task.isCancelled else { return }
            await MainActor.run { [weak self] in
                guard let self else { return }
                self.image = loaded
                self.errorMessage = loaded == nil ? "Unable to load image." : nil
            }
        }
    }

    private func installFileWatcher() {
        fileWatcher = nil
        fileWatcher = EditorFileWatcher(filePath: filePath) { [weak self] in
            Task { @MainActor [weak self] in
                self?.loadImage()
            }
        }
    }
}

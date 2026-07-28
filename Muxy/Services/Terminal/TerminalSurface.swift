import AppKit
import Foundation
import MuxyShared

struct TerminalCapabilities: OptionSet, Sendable {
    let rawValue: UInt64

    static let rawOutput = TerminalCapabilities(rawValue: 1 << 0)
    static let gridSnapshot = TerminalCapabilities(rawValue: 1 << 1)
    static let clientTheme = TerminalCapabilities(rawValue: 1 << 2)
    static let offlineLifecycle = TerminalCapabilities(rawValue: 1 << 3)
    static let search = TerminalCapabilities(rawValue: 1 << 4)
    static let imagePaste = TerminalCapabilities(rawValue: 1 << 5)

    static let ghostty: TerminalCapabilities = [
        .rawOutput,
        .gridSnapshot,
        .clientTheme,
        .offlineLifecycle,
        .search,
        .imagePaste,
    ]
}

enum TerminalSearchDirection: String {
    case next
    case previous
}

enum TerminalImagePasteAttempt: Equatable, Sendable {
    case local(surfaceGeneration: Int)
    case remote(RemoteImagePasteAttempt)
}

@MainActor
protocol TerminalRawOutputSource: AnyObject {
    func setRawOutputHandler(_ handler: ((Data) -> Void)?)
}

@MainActor
protocol TerminalGridSnapshotSource: AnyObject {
    func terminalCells(paneID: UUID) -> TerminalCellsDTO?
}

@MainActor
protocol TerminalClientThemeSurface: AnyObject {
    func applyClientTheme(_ theme: ClientThemeDTO?)
    func reapplyClientThemeIfOwned()
}

@MainActor
protocol TerminalOfflineSurface: AnyObject {
    var onOfflineChange: ((Bool) -> Void)? { get set }
    var isTakenOffline: Bool { get }
    var offlineInvisibleSince: Date? { get }
    var isOfflineBlockedByRemote: Bool { get }

    func wake()
    func isTerminalIdle() -> Bool
    func takeOffline()
}

@MainActor
protocol TerminalSearchSurface: AnyObject {
    var onSearchStart: ((String?) -> Void)? { get set }
    var onSearchEnd: (() -> Void)? { get set }
    var onSearchTotal: ((Int?) -> Void)? { get set }
    var onSearchSelected: ((Int?) -> Void)? { get set }

    func sendSearchQuery(_ needle: String)
    func navigateSearch(direction: TerminalSearchDirection)
    func endSearch()
    func startSearch()
}

@MainActor
protocol TerminalImagePasteSurface: AnyObject {
    var imagePasteWorkspaceContext: WorkspaceContext { get }

    func beginImagePaste() -> TerminalImagePasteAttempt?
    func pasteImageData(_ pngData: Data, attempt: TerminalImagePasteAttempt) async -> Bool
}

@MainActor
protocol TerminalInputSubmissionTarget: AnyObject {
    func sendRemoteBytes(_ bytes: Data)
    func submitRichInput(text: String)
    func clearTerminalInput(lineBreakCount: Int)
}

@MainActor
protocol TerminalInputTransactionTarget: TerminalInputSubmissionTarget {
    func enqueueInputTransaction(
        _ operation: @escaping @MainActor () async -> Bool
    ) -> TerminalInputTransactionHandle
}

@MainActor
protocol TerminalSurface: TerminalInputTransactionTarget {
    var terminalView: NSView { get }
    var backend: TerminalBackend { get }
    var envVars: [(key: String, value: String)] { get set }
    var onTitleChange: ((String) -> Void)? { get set }
    var onWorkingDirectoryChange: ((String) -> Void)? { get set }
    var onFocus: (() -> Void)? { get set }
    var onExternalDragHoverChange: ((Bool) -> Void)? { get set }
    var onProcessExit: (() -> Void)? { get set }
    var onSplitRequest: ((SplitDirection, SplitPosition) -> Void)? { get set }
    var onProgressReport: ((TerminalProgress?) -> Void)? { get set }
    var onCmdClickFile: ((String) -> Void)? { get set }
    var resolveCmdHoverFile: ((String) -> Bool)? { get set }
    var onOpenURL: ((URL) -> Bool)? { get set }
    var onDetectedAgentChange: ((String?) -> Void)? { get set }
    var onAgentProcessExit: (() -> Void)? { get set }
    var isFocused: Bool { get set }
    var overlayActive: Bool { get set }
    var foregroundProcessID: Int32? { get }
    var hasLiveSurface: Bool { get }

    func tearDown()
    func setVisible(_ visible: Bool)
    func setFocused(_ focused: Bool)
    func notifySurfaceUnfocused()
    func updateResumeWorkingDirectory(_ directory: String)
    func needsConfirmQuit() -> Bool
    func applyColorScheme(isDark: Bool)
    func reapplyActiveColors()
    func remoteOwnershipDidChange()
    func materializeHeadless()
    func ensureLiveSurfaceForExternalIO() -> Bool
    func sendText(_ text: String)
    func sendReturnKey()
    func readScreenText(lastLines: Int) -> String
    func scheduleTerminationCleanup()
    func scrollTerminal(deltaX: Double, deltaY: Double, precise: Bool)
    func resizeTerminal(cols: UInt32, rows: UInt32) -> Bool
}

@MainActor
extension TerminalSurface {
    var capabilities: TerminalCapabilities {
        var capabilities: TerminalCapabilities = []
        if self is any TerminalRawOutputSource {
            capabilities.insert(.rawOutput)
        }
        if self is any TerminalGridSnapshotSource {
            capabilities.insert(.gridSnapshot)
        }
        if self is any TerminalClientThemeSurface {
            capabilities.insert(.clientTheme)
        }
        if self is any TerminalOfflineSurface {
            capabilities.insert(.offlineLifecycle)
        }
        if self is any TerminalSearchSurface {
            capabilities.insert(.search)
        }
        if self is any TerminalImagePasteSurface {
            capabilities.insert(.imagePaste)
        }
        return capabilities
    }
}

enum TerminalSurfaceSizing {
    static func pixelSize(
        cols: UInt32,
        rows: UInt32,
        cellWidth: UInt32,
        cellHeight: UInt32
    ) -> (width: UInt32, height: UInt32)? {
        guard cols > 0, rows > 0, cellWidth > 0, cellHeight > 0 else { return nil }
        let width = cols.multipliedReportingOverflow(by: cellWidth)
        let height = rows.multipliedReportingOverflow(by: cellHeight)
        guard !width.overflow, !height.overflow else { return nil }
        return (width.partialValue, height.partialValue)
    }
}

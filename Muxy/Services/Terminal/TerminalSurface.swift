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

@MainActor
protocol TerminalRawOutputSource: AnyObject {
    func setRawOutputHandler(_ handler: ((Data) -> Void)?)
}

@MainActor
protocol TerminalSurface: TerminalRawOutputSource {
    var terminalView: NSView { get }
    var backend: TerminalBackend { get }
    var capabilities: TerminalCapabilities { get }
    var envVars: [(key: String, value: String)] { get set }
    var onTitleChange: ((String) -> Void)? { get set }
    var onWorkingDirectoryChange: ((String) -> Void)? { get set }
    var onFocus: (() -> Void)? { get set }
    var onExternalDragHoverChange: ((Bool) -> Void)? { get set }
    var onProcessExit: (() -> Void)? { get set }
    var onSplitRequest: ((SplitDirection, SplitPosition) -> Void)? { get set }
    var onSearchStart: ((String?) -> Void)? { get set }
    var onSearchEnd: (() -> Void)? { get set }
    var onSearchTotal: ((Int?) -> Void)? { get set }
    var onSearchSelected: ((Int?) -> Void)? { get set }
    var onProgressReport: ((TerminalProgress?) -> Void)? { get set }
    var onCmdClickFile: ((String) -> Void)? { get set }
    var resolveCmdHoverFile: ((String) -> Bool)? { get set }
    var onOpenURL: ((URL) -> Bool)? { get set }
    var onOfflineChange: ((Bool) -> Void)? { get set }
    var onDetectedAgentChange: ((String?) -> Void)? { get set }
    var onAgentProcessExit: (() -> Void)? { get set }
    var isFocused: Bool { get set }
    var overlayActive: Bool { get set }
    var processExitHandled: Bool { get set }
    var foregroundProcessID: Int32? { get }
    var isTakenOffline: Bool { get }
    var offlineInvisibleSince: Date? { get }
    var isOfflineBlockedByRemote: Bool { get }
    var hasLiveSurface: Bool { get }

    func tearDown()
    func setVisible(_ visible: Bool)
    func setFocused(_ focused: Bool)
    func notifySurfaceUnfocused()
    func wake()
    func updateResumeWorkingDirectory(_ directory: String)
    func isTerminalIdle() -> Bool
    func takeOffline()
    func needsConfirmQuit() -> Bool
    func applyColorScheme(isDark: Bool)
    func applyClientTheme(_ theme: ClientThemeDTO?)
    func reapplyActiveColors()
    func reapplyClientThemeIfOwned()
    func remoteOwnershipDidChange()
    func materializeHeadless()
    func ensureLiveSurfaceForExternalIO() -> Bool
    func sendText(_ text: String)
    func sendReturnKey()
    func sendRemoteBytes(_ bytes: Data)
    func readScreenText(lastLines: Int) -> String
    func submitRichInput(text: String)
    func clearTerminalInput()
    func pasteImageURL(_ url: URL)
    func sendSearchQuery(_ needle: String)
    func navigateSearch(direction: TerminalSearchDirection)
    func endSearch()
    func startSearch()
    func scrollTerminal(deltaX: Double, deltaY: Double, precise: Bool)
    func resizeTerminal(cols: UInt32, rows: UInt32) -> Bool
    func terminalCells(paneID: UUID) -> TerminalCellsDTO?
}

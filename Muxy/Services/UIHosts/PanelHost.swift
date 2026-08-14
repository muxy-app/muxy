import Foundation

struct PanelPlacement: Equatable {
    let panelID: String
    let position: PanelPosition
    let mode: PanelMode
}

@MainActor
@Observable
final class PanelHost {
    static let shared = PanelHost(
        modePreferences: MuxyFileStorage.isTestProcess ? nil : PanelModePreferences()
    )

    private(set) var placements: [PanelPlacement] = []

    var onDisplace: ((String) -> Void)?

    private let modePreferences: PanelModePreferences?

    init(modePreferences: PanelModePreferences? = nil) {
        self.modePreferences = modePreferences
    }

    func placement(for panelID: String) -> PanelPlacement? {
        placements.first { $0.panelID == panelID }
    }

    func isOpen(_ panelID: String) -> Bool {
        placement(for: panelID) != nil
    }

    func panel(at position: PanelPosition, mode: PanelMode) -> PanelPlacement? {
        placements.first { $0.position == position && $0.mode == mode }
    }

    func pinnedPanel(at position: PanelPosition) -> String? {
        panel(at: position, mode: .pinned)?.panelID
    }

    func floatingPanel(at position: PanelPosition) -> String? {
        panel(at: position, mode: .floating)?.panelID
    }

    func open(
        _ panelID: String,
        at position: PanelPosition,
        mode defaultMode: PanelMode,
        usesPreferredMode: Bool = true
    ) {
        let mode = resolvedMode(for: panelID, default: defaultMode, usesPreferredMode: usesPreferredMode)
        placements.removeAll { $0.panelID == panelID }
        let displaced = placements.filter { $0.position == position && $0.mode == mode }
        placements.removeAll { $0.position == position && $0.mode == mode }
        placements.append(PanelPlacement(panelID: panelID, position: position, mode: mode))
        displaced.forEach { onDisplace?($0.panelID) }
    }

    func toggle(_ panelID: String, at position: PanelPosition, mode: PanelMode) {
        if placement(for: panelID) != nil {
            close(panelID)
            return
        }
        open(panelID, at: position, mode: mode)
    }

    func move(_ panelID: String, to position: PanelPosition) {
        guard let current = placement(for: panelID) else { return }
        open(panelID, at: position, mode: current.mode)
    }

    func setMode(_ mode: PanelMode, for panelID: String) {
        guard let current = placement(for: panelID) else { return }
        modePreferences?.setMode(mode, for: panelID)
        open(panelID, at: current.position, mode: mode)
    }

    func close(_ panelID: String) {
        placements.removeAll { $0.panelID == panelID }
    }

    func closeAll() {
        placements.removeAll()
    }

    func resolvedMode(
        for panelID: String,
        default defaultMode: PanelMode,
        usesPreferredMode: Bool
    ) -> PanelMode {
        guard usesPreferredMode else { return defaultMode }
        return modePreferences?.mode(for: panelID, default: defaultMode) ?? defaultMode
    }
}

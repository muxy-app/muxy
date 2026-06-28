import AppKit
import SwiftUI

@MainActor
protocol ProjectPickerDefaultLocationPanel {
    func selectDirectory(initialPath: String, message: String) -> URL?
}

struct NSOpenPanelProjectPickerDefaultLocationPanel: ProjectPickerDefaultLocationPanel {
    func selectDirectory(initialPath: String, message: String) -> URL? {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.message = message
        panel.directoryURL = URL(fileURLWithPath: initialPath, isDirectory: true)
        guard panel.runModal() == .OK else { return nil }
        return panel.url
    }
}

@MainActor
@Observable
final class ProjectPickerDefaultLocationSettingsModel {
    private(set) var state: ProjectPickerDefaultLocationState
    private(set) var focusRequestID = 0

    @ObservationIgnored private let defaults: UserDefaults
    @ObservationIgnored private let pathService: ProjectPickerPathService
    @ObservationIgnored private let panel: any ProjectPickerDefaultLocationPanel
    @ObservationIgnored private let focusCoordinator: SettingsFocusCoordinator

    init(
        defaults: UserDefaults = .standard,
        pathService: ProjectPickerPathService = ProjectPickerPathService(),
        panel: any ProjectPickerDefaultLocationPanel = NSOpenPanelProjectPickerDefaultLocationPanel(),
        focusCoordinator: SettingsFocusCoordinator = .shared
    ) {
        self.defaults = defaults
        self.pathService = pathService
        self.panel = panel
        self.focusCoordinator = focusCoordinator
        state = ProjectPickerDefaultLocation.state(defaults: defaults, pathService: pathService)
    }

    var isResetDisabled: Bool {
        state.usesAppDefault
    }

    func refresh() {
        state = ProjectPickerDefaultLocation.state(defaults: defaults, pathService: pathService)
    }

    func reset() {
        ProjectPickerDefaultLocation.resetToAppDefault(defaults: defaults)
        refresh()
    }

    func chooseFolder() {
        guard let url = panel.selectDirectory(
            initialPath: state.chooserInitialPath,
            message: "Select the default location for the project picker"
        )
        else { return }
        ProjectPickerDefaultLocation.setCustomPath(from: url, defaults: defaults)
        refresh()
    }

    func handleAppActivation() {
        refresh()
    }

    func consumeFocusRequest() {
        guard focusCoordinator.consume(.projectPickerDefaultLocation) else { return }
        focusRequestID += 1
    }
}

struct ProjectPickerDefaultLocationSettingsView: View {
    let model: ProjectPickerDefaultLocationSettingsModel
    let pickerModeRaw: String
    @FocusState private var focusedControl: ProjectPickerDefaultLocationSettingsFocusedControl?

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Default path for project picker")
                .font(.system(size: SettingsMetrics.labelFontSize))

            HStack(alignment: .center, spacing: 8) {
                display
                    .layoutPriority(1)

                Button("Choose Folder...") {
                    model.chooseFolder()
                }
                .fixedSize(horizontal: true, vertical: false)
                .focused($focusedControl, equals: .chooseFolder)

                Button("Use App Default") {
                    model.reset()
                }
                .fixedSize(horizontal: true, vertical: false)
                .disabled(model.isResetDisabled)
            }

            if let warning = model.state.warning {
                Label(warning, systemImage: "exclamationmark.triangle.fill")
                    .font(.system(size: SettingsMetrics.footnoteFontSize))
                    .foregroundStyle(.orange)
            }
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
        .onAppear {
            model.refresh()
            model.consumeFocusRequest()
        }
        .onChange(of: pickerModeRaw) {
            model.refresh()
        }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            model.handleAppActivation()
        }
        .onReceive(NotificationCenter.default.publisher(for: .focusProjectPickerDefaultLocation)) { _ in
            model.consumeFocusRequest()
        }
        .onChange(of: model.focusRequestID) {
            focusChooseFolder()
        }
        .resetsSettingsFocusOnOutsideClick()
    }

    private var display: some View {
        HStack(spacing: 7) {
            Image(systemName: "folder")
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(.secondary)
                .frame(width: 15)

            Text(model.state.displayPath)
                .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                .foregroundStyle(model.state.usesAppDefault ? .secondary : .primary)
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.horizontal, 9)
        .frame(minWidth: 170, maxWidth: .infinity, alignment: .leading)
        .frame(height: 22)
        .background(.quaternary.opacity(0.45), in: RoundedRectangle(cornerRadius: 6))
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(.quaternary.opacity(0.7), lineWidth: 1)
        )
    }

    private func focusChooseFolder() {
        focusedControl = nil
        DispatchQueue.main.async {
            focusedControl = .chooseFolder
        }
    }
}

private enum ProjectPickerDefaultLocationSettingsFocusedControl: Hashable {
    case chooseFolder
}

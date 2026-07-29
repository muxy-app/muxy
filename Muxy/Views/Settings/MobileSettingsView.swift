import AppKit
import Network
import SwiftUI

struct MobileSettingsView: View {
    private static let pairingFooter: LocalizedStringResource = """
    Scan this with the Muxy mobile app to add this Mac. \
    The QR carries no token — first-time pairing still needs your approval.
    """

    @Bindable private var service = MobileServerService.shared
    @Bindable private var devices = ApprovedDevicesStore.shared
    @State private var deviceToRevoke: ApprovedDevice?
    @State private var isSelecting = false
    @State private var selectedDeviceIDs: Set<UUID> = []
    @State private var showBatchRevokeConfirmation = false
    @State private var portText: String = ""
    @State private var portValidationError: String?
    @State private var showFreePortConfirmation = false
    @State private var didCopyPairingLink = false
    @State private var pairingHosts: [MobilePairingHost] = []
    @State private var selectedNetwork: MobilePairingNetwork = .local
    @State private var pathMonitor: NWPathMonitor?

    private var enabledBinding: Binding<Bool> {
        Binding(
            get: { service.isEnabled },
            set: { newValue in
                if newValue, !commitPort() {
                    return
                }
                service.setEnabled(newValue)
            }
        )
    }

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Mobile",
                footer: L10n
                    .resource(
                        "Muxy listens on the configured port for the iOS app over your local network or a private VPN such as Tailscale."
                    )
            ) {
                SettingsToggleRow(label: L10n.resource("Allow mobile device connections"), isOn: enabledBinding)

                SettingsRow("Port") {
                    TextField(L10n.string("\(MobileServerService.defaultPort)"), text: $portText)
                        .font(.system(size: SettingsMetrics.labelFontSize, design: .monospaced))
                        .settingsTextInput(width: SettingsMetrics.controlWidth)
                        .onChange(of: portText) { _, _ in
                            guard portText != String(service.port) else { return }
                            portValidationError = nil
                            if service.isEnabled {
                                service.setEnabled(false)
                            }
                        }
                        .onSubmit { _ = commitPort() }
                }

                if let error = portValidationError ?? service.lastError {
                    HStack(spacing: 6) {
                        Text(error)
                            .font(.system(size: SettingsMetrics.footnoteFontSize))
                            .foregroundStyle(SettingsStyle.destructive)
                            .fixedSize(horizontal: false, vertical: true)
                        if service.isPortInUse {
                            Button(L10n.string("Free Port")) {
                                showFreePortConfirmation = true
                            }
                            .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium))
                            .buttonStyle(.borderless)
                            .foregroundStyle(MuxyTheme.accent)
                        }
                    }
                    .padding(.horizontal, SettingsMetrics.horizontalPadding)
                    .padding(.vertical, SettingsMetrics.rowVerticalPadding)
                }
            }

            if service.isEnabled, let selectedHost, let uri = pairingURI(for: selectedHost) {
                SettingsSection(
                    "Pair Mobile Device",
                    footer: Self.pairingFooter
                ) {
                    pairingCard(host: selectedHost, uri: uri)
                }
            }

            SettingsSection(
                "Approved Devices",
                footer: L10n.resource("Revoking removes the device's access. It will need to request approval again to reconnect."),
                showsDivider: false
            ) {
                if devices.devices.isEmpty {
                    Text(L10n.resource("No devices approved yet."))
                        .font(.system(size: SettingsMetrics.labelFontSize))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                        .padding(.horizontal, SettingsMetrics.horizontalPadding)
                        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
                } else {
                    deviceSelectionActions
                    ForEach(devices.devices) { device in
                        deviceRow(device)
                    }
                }
            }
        }
        .onAppear {
            portText = String(service.port)
            refreshPairingHosts()
            startPathMonitor()
        }
        .onDisappear { stopPathMonitor() }
        .onChange(of: service.port) { _, newValue in
            let text = String(newValue)
            if portText != text {
                portText = text
            }
        }
        .onChange(of: service.isEnabled) { _, _ in
            refreshPairingHosts()
        }
        .onChange(of: devices.devices) { _, newValue in
            selectedDeviceIDs.formIntersection(Set(newValue.map(\.id)))
            if isSelecting, newValue.isEmpty {
                exitSelection()
            }
        }
        .alert(
            L10n.string("Free port \(String(service.port))?"),
            isPresented: $showFreePortConfirmation
        ) {
            Button(L10n.string("Free Port"), role: .destructive) {
                service.freePort()
            }
            Button(L10n.string("Cancel"), role: .cancel) {}
        } message: {
            Text(L10n.resource("This will terminate any process currently listening on port \(String(service.port))."))
        }
        .alert(
            L10n.string("Revoke \(deviceToRevoke?.name ?? "device")?"),
            isPresented: Binding(
                get: { deviceToRevoke != nil },
                set: {
                    if !$0 {
                        deviceToRevoke = nil
                    }
                }
            ),
            presenting: deviceToRevoke
        ) { device in
            Button(L10n.string("Revoke"), role: .destructive) {
                devices.revoke(deviceID: device.id)
            }
            Button(L10n.string("Cancel"), role: .cancel) {}
        } message: { _ in
            Text(L10n.resource("The device will be disconnected immediately and must request approval again to reconnect."))
        }
        .alert(
            selectedDeviceIDs.count == 1
                ? L10n.string("Revoke 1 device?")
                : L10n.string("Revoke \(selectedDeviceIDs.count) devices?"),
            isPresented: $showBatchRevokeConfirmation
        ) {
            Button(L10n.string("Revoke"), role: .destructive) {
                devices.revoke(deviceIDs: selectedDeviceIDs)
                exitSelection()
            }
            Button(L10n.string("Cancel"), role: .cancel) {}
        } message: {
            Text(L10n.resource("The selected devices will be disconnected immediately and must request approval again to reconnect."))
        }
    }

    private func commitPort() -> Bool {
        let trimmed = portText.trimmingCharacters(in: .whitespaces)
        guard let value = UInt16(trimmed), MobileServerService.isValid(port: value) else {
            portValidationError = L10n.string(
                "Enter a port between \(MobileServerService.minPort) and \(MobileServerService.maxPort)."
            )
            return false
        }
        portValidationError = nil
        service.port = value
        portText = String(value)
        return true
    }

    private var selectedHost: MobilePairingHost? {
        pairingHosts.first(where: { $0.network == selectedNetwork }) ?? pairingHosts.first
    }

    private func pairingURI(for host: MobilePairingHost) -> String? {
        MobilePairingService.pairingURIString(for: host, port: service.port)
    }

    private func refreshPairingHosts() {
        pairingHosts = MobilePairingService.availableHosts()
        if !pairingHosts.contains(where: { $0.network == selectedNetwork }) {
            selectedNetwork = pairingHosts.first?.network ?? .local
        }
    }

    private func startPathMonitor() {
        guard pathMonitor == nil else { return }
        let monitor = NWPathMonitor()
        monitor.pathUpdateHandler = { _ in
            Task { @MainActor in refreshPairingHosts() }
        }
        monitor.start(queue: .global(qos: .utility))
        pathMonitor = monitor
    }

    private func stopPathMonitor() {
        pathMonitor?.cancel()
        pathMonitor = nil
    }

    private func pairingCard(host: MobilePairingHost, uri: String) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            if pairingHosts.count > 1 {
                Picker(L10n.string("Pairing network"), selection: $selectedNetwork) {
                    ForEach(pairingHosts, id: \.network) { option in
                        Text(L10n.resource(key: option.network.displayName)).tag(option.network)
                    }
                }
                .labelsHidden()
                .pickerStyle(.segmented)
                .fixedSize()
                .frame(maxWidth: .infinity, alignment: .leading)
                .accessibilityLabel(L10n.string("Pairing network"))
            }

            HStack(alignment: .top, spacing: 14) {
                MobilePairingQRView(uriString: uri, size: 132)
                VStack(alignment: .leading, spacing: 6) {
                    Text(L10n.resource("Open the Muxy mobile app, tap Add device, and scan this code."))
                        .font(.system(size: SettingsMetrics.labelFontSize))
                        .fixedSize(horizontal: false, vertical: true)
                    Text(host.host)
                        .font(.system(size: SettingsMetrics.labelFontSize, design: .monospaced))
                        .foregroundStyle(SettingsStyle.foreground)
                        .textSelection(.enabled)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Text(L10n.resource("Port \(String(service.port))"))
                        .font(.system(size: SettingsMetrics.footnoteFontSize))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                }
                Spacer(minLength: 0)
            }

            pairingLinkRow(uri: uri)
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }

    private func pairingLinkRow(uri: String) -> some View {
        HStack(spacing: 8) {
            Text(uri)
                .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                .foregroundStyle(SettingsStyle.mutedForeground)
                .textSelection(.enabled)
                .lineLimit(1)
                .truncationMode(.middle)
                .frame(maxWidth: .infinity, alignment: .leading)
            Button {
                copyPairingLink(uri)
            } label: {
                Label(
                    didCopyPairingLink
                        ? L10n.string("Copied")
                        : L10n.string("Copy"),
                    systemImage: didCopyPairingLink ? "checkmark" : "doc.on.doc"
                )
                .labelStyle(.titleAndIcon)
                .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium))
            }
            .buttonStyle(.borderless)
            .foregroundStyle(MuxyTheme.accent)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: 6))
    }

    private func copyPairingLink(_ uri: String) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(uri, forType: .string)
        didCopyPairingLink = true
        Task {
            try? await Task.sleep(for: .seconds(2))
            await MainActor.run { didCopyPairingLink = false }
        }
    }

    private var deviceSelectionActions: some View {
        HStack(spacing: 12) {
            if isSelecting {
                Button(
                    allDevicesSelected
                        ? L10n.string("Deselect All")
                        : L10n.string("Select All")
                ) {
                    toggleSelectAll()
                }
                .buttonStyle(.borderless)
                .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium))
                .foregroundStyle(MuxyTheme.accent)

                Button(L10n.string("Revoke Selected (\(selectedDeviceIDs.count))"), role: .destructive) {
                    showBatchRevokeConfirmation = true
                }
                .buttonStyle(.borderless)
                .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium))
                .foregroundStyle(SettingsStyle.destructive)
                .disabled(selectedDeviceIDs.isEmpty)

                Spacer()

                Button(L10n.string("Done")) {
                    exitSelection()
                }
                .buttonStyle(.borderless)
                .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium))
                .foregroundStyle(MuxyTheme.accent)
            } else {
                Spacer()
                Button(L10n.string("Select")) {
                    isSelecting = true
                }
                .buttonStyle(.borderless)
                .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium))
                .foregroundStyle(MuxyTheme.accent)
            }
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }

    private var allDevicesSelected: Bool {
        !devices.devices.isEmpty && selectedDeviceIDs.count == devices.devices.count
    }

    private func toggleSelectAll() {
        if allDevicesSelected {
            selectedDeviceIDs.removeAll()
        } else {
            selectedDeviceIDs = Set(devices.devices.map(\.id))
        }
    }

    private func exitSelection() {
        isSelecting = false
        selectedDeviceIDs.removeAll()
    }

    private func toggleSelection(_ device: ApprovedDevice) {
        if selectedDeviceIDs.contains(device.id) {
            selectedDeviceIDs.remove(device.id)
        } else {
            selectedDeviceIDs.insert(device.id)
        }
    }

    private func deviceRow(_ device: ApprovedDevice) -> some View {
        HStack {
            if isSelecting {
                Image(systemName: selectedDeviceIDs.contains(device.id) ? "checkmark.circle.fill" : "circle")
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .foregroundStyle(
                        selectedDeviceIDs.contains(device.id) ? MuxyTheme.accent : SettingsStyle.mutedForeground
                    )
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(device.name)
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .lineLimit(1)
                    .truncationMode(.tail)
                Text(lastSeenText(device))
                    .font(.system(size: SettingsMetrics.footnoteFontSize))
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .lineLimit(1)
            }
            Spacer(minLength: SettingsMetrics.rowSpacing)
            if !isSelecting {
                Button(L10n.string("Revoke"), role: .destructive) {
                    deviceToRevoke = device
                }
                .buttonStyle(.borderless)
                .font(.system(size: SettingsMetrics.footnoteFontSize))
                .foregroundStyle(SettingsStyle.destructive)
            }
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
        .contentShape(Rectangle())
        .onTapGesture {
            guard isSelecting else { return }
            toggleSelection(device)
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(isSelecting ? .isButton : [])
        .accessibilityAddTraits(isSelecting && selectedDeviceIDs.contains(device.id) ? .isSelected : [])
    }

    private func lastSeenText(_ device: ApprovedDevice) -> String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .short
        if let seen = device.lastSeenAt {
            return L10n.string("Last seen \(formatter.localizedString(for: seen, relativeTo: Date()))")
        }
        return L10n.string("Approved \(formatter.localizedString(for: device.approvedAt, relativeTo: Date()))")
    }
}

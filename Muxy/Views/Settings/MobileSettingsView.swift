import AppKit
import Network
import SwiftUI

struct MobileSettingsView: View {
    private static let pairingFooter = """
    Scan this with the Muxy mobile app to add this Mac. \
    The QR carries no token — first-time pairing still needs your approval.
    """

    @Bindable private var service = MobileServerService.shared
    @Bindable private var devices = ApprovedDevicesStore.shared
    @State private var deviceToRevoke: ApprovedDevice?
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
                if newValue, !commitPort() { return }
                service.setEnabled(newValue)
            }
        )
    }

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Mobile",
                footer: "Muxy listens on the configured port for the iOS app over your local network or a private VPN such as Tailscale."
            ) {
                SettingsToggleRow(label: "Allow mobile device connections", isOn: enabledBinding)

                SettingsRow("Port") {
                    TextField("\(MobileServerService.defaultPort)", text: $portText)
                        .textFieldStyle(.roundedBorder)
                        .font(.system(size: SettingsMetrics.labelFontSize, design: .monospaced))
                        .frame(width: SettingsMetrics.controlWidth)
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
                            .foregroundStyle(.red)
                            .fixedSize(horizontal: false, vertical: true)
                        if service.isPortInUse {
                            Button("Free Port") {
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
                footer: "Revoking removes the device's access. It will need to request approval again to reconnect.",
                showsDivider: false
            ) {
                if devices.devices.isEmpty {
                    Text("No devices approved yet.")
                        .font(.system(size: SettingsMetrics.labelFontSize))
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, SettingsMetrics.horizontalPadding)
                        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
                } else {
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
            if portText != text { portText = text }
        }
        .onChange(of: service.isEnabled) { _, _ in
            refreshPairingHosts()
        }
        .alert(
            "Free port \(String(service.port))?",
            isPresented: $showFreePortConfirmation
        ) {
            Button("Free Port", role: .destructive) {
                service.freePort()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This will terminate any process currently listening on port \(String(service.port)).")
        }
        .alert(
            "Revoke \(deviceToRevoke?.name ?? "device")?",
            isPresented: Binding(
                get: { deviceToRevoke != nil },
                set: { if !$0 { deviceToRevoke = nil } }
            ),
            presenting: deviceToRevoke
        ) { device in
            Button("Revoke", role: .destructive) {
                devices.revoke(deviceID: device.id)
            }
            Button("Cancel", role: .cancel) {}
        } message: { _ in
            Text("The device will be disconnected immediately and must request approval again to reconnect.")
        }
    }

    private func commitPort() -> Bool {
        let trimmed = portText.trimmingCharacters(in: .whitespaces)
        guard let value = UInt16(trimmed), MobileServerService.isValid(port: value) else {
            portValidationError = "Enter a port between \(MobileServerService.minPort) and \(MobileServerService.maxPort)."
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
                Picker("Pairing network", selection: $selectedNetwork) {
                    ForEach(pairingHosts, id: \.network) { option in
                        Text(option.network.displayName).tag(option.network)
                    }
                }
                .labelsHidden()
                .pickerStyle(.segmented)
                .fixedSize()
                .frame(maxWidth: .infinity, alignment: .leading)
                .accessibilityLabel("Pairing network")
            }

            HStack(alignment: .top, spacing: 14) {
                MobilePairingQRView(uriString: uri, size: 132)
                VStack(alignment: .leading, spacing: 6) {
                    Text("Open the Muxy mobile app, tap Add device, and scan this code.")
                        .font(.system(size: SettingsMetrics.labelFontSize))
                        .fixedSize(horizontal: false, vertical: true)
                    Text(host.host)
                        .font(.system(size: SettingsMetrics.labelFontSize, design: .monospaced))
                        .foregroundStyle(.primary)
                        .textSelection(.enabled)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Text("Port \(String(service.port))")
                        .font(.system(size: SettingsMetrics.footnoteFontSize))
                        .foregroundStyle(.secondary)
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
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
                .lineLimit(1)
                .truncationMode(.middle)
                .frame(maxWidth: .infinity, alignment: .leading)
            Button {
                copyPairingLink(uri)
            } label: {
                Label(
                    didCopyPairingLink ? "Copied" : "Copy",
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
        .background(.quaternary, in: RoundedRectangle(cornerRadius: 6))
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

    private func deviceRow(_ device: ApprovedDevice) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(device.name)
                    .font(.system(size: SettingsMetrics.labelFontSize))
                Text(lastSeenText(device))
                    .font(.system(size: SettingsMetrics.footnoteFontSize))
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Button("Revoke", role: .destructive) {
                deviceToRevoke = device
            }
            .buttonStyle(.borderless)
            .font(.system(size: SettingsMetrics.footnoteFontSize))
            .foregroundStyle(.red)
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }

    private func lastSeenText(_ device: ApprovedDevice) -> String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .short
        if let seen = device.lastSeenAt {
            return "Last seen \(formatter.localizedString(for: seen, relativeTo: Date()))"
        }
        return "Approved \(formatter.localizedString(for: device.approvedAt, relativeTo: Date()))"
    }
}

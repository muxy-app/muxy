import SwiftUI

struct MobileSettingsView: View {
    @Bindable private var service = MobileServerService.shared
    @Bindable private var devices = ApprovedDevicesStore.shared
    @State private var deviceToRevoke: ApprovedDevice?

    private var enabledBinding: Binding<Bool> {
        Binding(
            get: { service.isEnabled },
            set: { service.setEnabled($0) }
        )
    }

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Mobile",
                footer: "Muxy listens on port 4865 for the iOS app over your local network or a private VPN such as Tailscale."
            ) {
                SettingsToggleRow(label: "Allow mobile device connections", isOn: enabledBinding)
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

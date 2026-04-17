import SwiftUI

struct MobileSettingsView: View {
    @Bindable private var service = MobileServerService.shared

    private var enabledBinding: Binding<Bool> {
        Binding(
            get: { service.isEnabled },
            set: { service.setEnabled($0) }
        )
    }

    var body: some View {
        Form {
            Section {
                Toggle("Allow mobile device connections", isOn: enabledBinding)
            } header: {
                Text("Mobile")
            } footer: {
                Text("When enabled, Muxy listens on port 4865 for the Muxy iOS app on the same network.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
        .padding()
    }
}

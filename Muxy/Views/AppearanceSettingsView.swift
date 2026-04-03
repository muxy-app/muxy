import SwiftUI

struct AppearanceSettingsView: View {
    @State private var themeService = ThemeService.shared
    @State private var showThemePicker = false
    @State private var currentTheme: String?

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Terminal Theme")
                    .font(.system(size: 12))

                Spacer()

                Button {
                    showThemePicker.toggle()
                } label: {
                    HStack(spacing: 6) {
                        Text(currentTheme ?? "Default")
                            .font(.system(size: 12))
                            .lineLimit(1)
                        Image(systemName: "chevron.up.chevron.down")
                            .font(.system(size: 10))
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(.quaternary, in: RoundedRectangle(cornerRadius: 6))
                }
                .buttonStyle(.plain)
                .popover(isPresented: $showThemePicker) {
                    ThemePicker()
                        .environment(themeService)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)

            Spacer()
        }
        .task {
            currentTheme = themeService.currentThemeName()
        }
        .onReceive(NotificationCenter.default.publisher(for: .themeDidChange)) { _ in
            currentTheme = themeService.currentThemeName()
        }
    }
}

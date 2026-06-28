import SwiftUI

struct WelcomeView: View {
    var body: some View {
        VStack(spacing: 0) {
            WindowDragRepresentable()
                .frame(height: UIMetrics.scaled(32))
            Spacer()
            Text("No project selected")
                .font(.system(size: UIMetrics.fontEmphasis))
                .foregroundStyle(MuxyTheme.fgDim)
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

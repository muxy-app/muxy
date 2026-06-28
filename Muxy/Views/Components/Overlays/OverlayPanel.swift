import SwiftUI

struct OverlayPanel<Content: View>: View {
    let width: CGFloat
    let height: CGFloat
    @ViewBuilder let content: () -> Content

    var body: some View {
        content()
            .frame(width: width, height: height)
            .background(MuxyTheme.bg)
            .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusXL))
            .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusXL).stroke(MuxyTheme.border, lineWidth: 1))
            .shadow(color: .black.opacity(0.4), radius: UIMetrics.scaled(20), y: UIMetrics.scaled(8))
            .padding(.top, UIMetrics.scaled(60))
            .frame(maxHeight: .infinity, alignment: .top)
            .accessibilityAddTraits(.isModal)
    }
}

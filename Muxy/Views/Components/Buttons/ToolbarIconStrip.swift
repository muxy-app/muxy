import SwiftUI

struct ToolbarIconStrip<Content: View>: View {
    @ViewBuilder let content: () -> Content

    var body: some View {
        HStack(spacing: 0) {
            content()
        }
        .padding(.trailing, UIMetrics.spacing2)
    }
}

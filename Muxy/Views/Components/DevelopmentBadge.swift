import SwiftUI

struct DevelopmentBadge: View {
    var body: some View {
        Text("DEV")
            .font(.system(size: 10, weight: .bold))
            .foregroundStyle(Color.black.opacity(0.85))
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(Color.orange, in: Capsule())
            .allowsHitTesting(false)
    }
}

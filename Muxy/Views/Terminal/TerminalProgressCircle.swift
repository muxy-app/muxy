import SwiftUI

struct TerminalProgressCircle: View {
    let progress: TerminalProgress
    var size: CGFloat = 12
    var lineWidth: CGFloat = 1.5

    @State private var rotation: Double = 0

    var body: some View {
        ZStack {
            Circle()
                .stroke(tintColor.opacity(0.25), lineWidth: lineWidth)

            if progress.kind == .indeterminate {
                Circle()
                    .trim(from: 0, to: 0.28)
                    .stroke(tintColor, style: StrokeStyle(lineWidth: lineWidth, lineCap: .round))
                    .rotationEffect(.degrees(rotation))
            } else {
                Circle()
                    .trim(from: 0, to: trimEnd)
                    .stroke(tintColor, style: StrokeStyle(lineWidth: lineWidth, lineCap: .round))
                    .rotationEffect(.degrees(-90))
                    .animation(.easeInOut(duration: 0.2), value: trimEnd)
            }
        }
        .frame(width: size, height: size)
        .onAppear {
            guard progress.kind == .indeterminate else { return }
            startSpin()
        }
        .onChange(of: progress.kind) { _, kind in
            guard kind == .indeterminate else { return }
            startSpin()
        }
        .accessibilityElement()
        .accessibilityLabel(accessibilityLabel)
    }

    private var trimEnd: CGFloat {
        let value = CGFloat(progress.percent ?? 100) / 100
        return max(0.001, min(1, value))
    }

    private var tintColor: Color {
        switch progress.kind {
        case .set,
             .indeterminate: MuxyTheme.accent
        case .error: Color(nsColor: .systemRed)
        case .paused: MuxyTheme.warning
        }
    }

    private var accessibilityLabel: String {
        switch progress.kind {
        case .set: "Progress \(progress.percent ?? 0) percent"
        case .error: "Progress error"
        case .indeterminate: "Working"
        case .paused: "Progress paused"
        }
    }

    private func startSpin() {
        rotation = 0
        withAnimation(.linear(duration: 1.0).repeatForever(autoreverses: false)) {
            rotation = 360
        }
    }
}

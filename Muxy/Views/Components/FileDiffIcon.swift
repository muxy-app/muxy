import SwiftUI

struct FileDiffIcon: Shape {
    func path(in rect: CGRect) -> Path {
        let s = min(rect.width, rect.height)
        let scale = s / 24.0
        let ox = rect.midX - s / 2
        let oy = rect.midY - s / 2

        func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint {
            CGPoint(x: ox + x * scale, y: oy + y * scale)
        }

        var path = Path()

        path.move(to: p(14, 2))
        path.addLine(to: p(6, 2))
        path.addQuadCurve(to: p(4, 4), control: p(4, 2))
        path.addLine(to: p(4, 20))
        path.addQuadCurve(to: p(6, 22), control: p(4, 22))
        path.addLine(to: p(18, 22))
        path.addQuadCurve(to: p(20, 20), control: p(20, 22))
        path.addLine(to: p(20, 8))
        path.addLine(to: p(14, 2))
        path.closeSubpath()

        path.move(to: p(9, 10))
        path.addLine(to: p(15, 10))

        path.move(to: p(12, 7))
        path.addLine(to: p(12, 13))

        path.move(to: p(9, 17))
        path.addLine(to: p(15, 17))

        return path
    }
}

struct FileDiffIconButton: View {
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            FileDiffIcon()
                .stroke(
                    hovered ? MuxyTheme.fg : MuxyTheme.fgMuted,
                    style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round)
                )
                .frame(width: UIMetrics.scaled(13), height: UIMetrics.scaled(13))
                .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .accessibilityLabel("Source Control")
    }
}

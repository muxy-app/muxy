import AppKit
import SwiftUI

private struct TooltipView: NSViewRepresentable {
    let tooltip: String?

    func makeNSView(context _: Context) -> NSView {
        let view = NSView()
        view.toolTip = tooltip
        return view
    }

    func updateNSView(_ nsView: NSView, context _: Context) {
        nsView.toolTip = tooltip
    }
}

private struct TruncationHelpModifier: ViewModifier {
    let text: String
    let font: NSFont
    @State private var availableWidth: CGFloat = 0

    private var isTruncated: Bool {
        guard availableWidth > 0 else { return false }
        let naturalWidth = (text as NSString).size(withAttributes: [.font: font]).width
        return naturalWidth > availableWidth + 0.5
    }

    func body(content: Content) -> some View {
        content
            .background(
                GeometryReader { proxy in
                    TooltipView(tooltip: isTruncated ? text : nil)
                        .onAppear { availableWidth = proxy.size.width }
                        .onChange(of: proxy.size.width) { _, width in availableWidth = width }
                }
            )
    }
}

extension View {
    func helpIfTruncated(_ text: String, font: NSFont) -> some View {
        modifier(TruncationHelpModifier(text: text, font: font))
    }
}

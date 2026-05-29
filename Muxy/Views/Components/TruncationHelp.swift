import SwiftUI

private struct TruncationPreferenceKey: PreferenceKey {
    static let defaultValue = false
    static func reduce(value: inout Bool, nextValue: () -> Bool) {
        value = nextValue()
    }
}

private struct TruncationHelpModifier: ViewModifier {
    let text: String
    @State private var isTruncated = false

    func body(content: Content) -> some View {
        content
            .overlay(
                GeometryReader { container in
                    Text(text)
                        .fixedSize(horizontal: true, vertical: false)
                        .hidden()
                        .overlay(
                            GeometryReader { natural in
                                Color.clear.preference(
                                    key: TruncationPreferenceKey.self,
                                    value: natural.size.width > container.size.width
                                )
                            }
                        )
                }
            )
            .onPreferenceChange(TruncationPreferenceKey.self) { isTruncated = $0 }
            .help(isTruncated ? text : "")
    }
}

extension View {
    func helpIfTruncated(_ text: String) -> some View {
        modifier(TruncationHelpModifier(text: text))
    }
}

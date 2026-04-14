import SwiftUI

extension View {
    func deferFocus(_ focus: FocusState<Bool>.Binding, on value: Int) -> some View {
        onAppear {
            DispatchQueue.main.async { focus.wrappedValue = true }
        }
        .onChange(of: value) { _, _ in
            DispatchQueue.main.async { focus.wrappedValue = true }
        }
    }
}

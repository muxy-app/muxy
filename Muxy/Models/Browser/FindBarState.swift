import Foundation

@MainActor
@Observable
final class FindBarState {
    var isVisible: Bool = false
    var query: String = ""
    var lastResultFound: Bool?
    var focusVersion: Int = 0
}

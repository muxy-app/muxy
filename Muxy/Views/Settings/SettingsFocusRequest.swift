import Foundation

@MainActor
enum SettingsFocusRequest {
    private static var pendingProjectPickerDefaultLocation = false

    static func requestProjectPickerDefaultLocation() {
        pendingProjectPickerDefaultLocation = true
        NotificationCenter.default.post(name: .focusProjectPickerDefaultLocation, object: nil)
    }

    static func consumeProjectPickerDefaultLocation() -> Bool {
        guard pendingProjectPickerDefaultLocation else { return false }
        pendingProjectPickerDefaultLocation = false
        return true
    }
}

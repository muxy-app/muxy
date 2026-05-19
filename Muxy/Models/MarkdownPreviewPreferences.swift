import Foundation

enum MarkdownPreviewPreferences {
    static let allowRemoteImagesKey = "muxy.markdown.allowRemoteImages"

    static var allowRemoteImages: Bool {
        get { UserDefaults.standard.bool(forKey: allowRemoteImagesKey, fallback: false) }
        set { UserDefaults.standard.set(newValue, forKey: allowRemoteImagesKey) }
    }
}

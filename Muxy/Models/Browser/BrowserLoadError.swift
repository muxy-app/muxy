import Foundation

struct BrowserLoadError: Equatable {
    let failedURL: String
    let message: String

    static func make(from error: Error, url: URL?) -> BrowserLoadError? {
        let nsError = error as NSError
        guard !(nsError.domain == NSURLErrorDomain && nsError.code == NSURLErrorCancelled) else {
            return nil
        }
        return BrowserLoadError(
            failedURL: url?.absoluteString ?? "",
            message: nsError.localizedDescription
        )
    }
}

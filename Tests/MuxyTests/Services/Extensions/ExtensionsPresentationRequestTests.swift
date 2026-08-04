import Foundation
import Testing

@testable import Muxy

@MainActor
@Suite("Extensions presentation request")
struct ExtensionsPresentationRequestTests {
    @Test("root request has no browse category")
    func rootRequest() {
        let request = ExtensionsPresentationRequest(
            Notification(name: .openExtensionsModal)
        )

        #expect(request == ExtensionsPresentationRequest())
    }

    @Test("browse request preserves its category across notifications")
    func browseRequest() {
        let notificationCenter = NotificationCenter()
        let expected = ExtensionsPresentationRequest(
            browseCategory: ExtensionMarketplaceCategory.localization
        )
        var received: ExtensionsPresentationRequest?
        let observer = notificationCenter.addObserver(
            forName: .openExtensionsModal,
            object: nil,
            queue: nil
        ) { notification in
            received = ExtensionsPresentationRequest(notification)
        }
        defer { notificationCenter.removeObserver(observer) }

        expected.post(notificationCenter: notificationCenter)

        #expect(received == expected)
    }

    @Test("browse handoff posts the category to an existing Extensions window")
    func browseHandoff() {
        let notificationCenter = NotificationCenter()
        let expected = ExtensionsPresentationRequest(
            browseCategory: ExtensionMarketplaceCategory.localization
        )
        var received: ExtensionsPresentationRequest?
        let observer = notificationCenter.addObserver(
            forName: .openExtensionBrowse,
            object: nil,
            queue: nil
        ) { notification in
            received = ExtensionsPresentationRequest(notification)
        }
        defer { notificationCenter.removeObserver(observer) }

        expected.postBrowse(notificationCenter: notificationCenter)

        #expect(received == expected)
    }
}

import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionWebviewModalService")
@MainActor
struct ExtensionWebviewModalServiceTests {
    private func request(
        extensionID: String = "ext",
        entry: String = "modal/form.html",
        width: Double? = nil,
        height: Double? = nil,
        dismissOnOutsideClick: Bool = true,
        data: ExtensionJSON? = nil
    ) -> ExtensionWebviewModalService.OpenRequest {
        ExtensionWebviewModalService.OpenRequest(
            extensionID: extensionID,
            entry: entry,
            width: width,
            height: height,
            dismissOnOutsideClick: dismissOnOutsideClick,
            data: data
        )
    }

    @Test("open exposes an active request with clamped size")
    func openExposesActiveRequest() async {
        let service = ExtensionWebviewModalService()
        let requestID = service.open(request(
            width: 10,
            height: 5000,
            dismissOnOutsideClick: false,
            data: .object(["name": .string("hi")])
        ))
        let active = service.active
        #expect(active?.id == requestID)
        #expect(active?.extensionID == "ext")
        #expect(active?.entry == "modal/form.html")
        #expect(active?.dismissOnOutsideClick == false)
        #expect(active?.width == ExtensionWebviewModalService.minSize)
        #expect(active?.height == ExtensionWebviewModalService.maxHeight)
    }

    @Test("submit resolves the opener with the payload and clears active")
    func submitResolvesWithPayload() async {
        let service = ExtensionWebviewModalService()
        let requestID = service.open(request())

        async let awaited = service.awaitClose(requestID: requestID)
        service.submit(requestID: requestID, result: .object(["value": .string("submitted")]))
        let result = await awaited

        #expect(result == .object(["value": .string("submitted")]))
        #expect(service.active == nil)
    }

    @Test("dismiss resolves the opener with nil")
    func dismissResolvesNil() async {
        let service = ExtensionWebviewModalService()
        let requestID = service.open(request())

        async let awaited = service.awaitClose(requestID: requestID)
        service.dismiss(requestID: requestID)
        let result = await awaited

        #expect(result == nil)
        #expect(service.active == nil)
    }

    @Test("submit before await is buffered and delivered on await")
    func submitBeforeAwaitIsBuffered() async {
        let service = ExtensionWebviewModalService()
        let requestID = service.open(request())

        service.forceClose(instanceID: requestID)
        let result = await service.awaitClose(requestID: requestID)

        #expect(result == nil)
        #expect(service.active == nil)
    }

    @Test("opening a second modal resolves the prior opener with nil")
    func openingSecondResolvesPrior() async {
        let service = ExtensionWebviewModalService()
        let first = service.open(request(entry: "a.html"))

        async let firstResult = service.awaitClose(requestID: first)
        let second = service.open(request(entry: "b.html"))
        let resolvedFirst = await firstResult

        #expect(resolvedFirst == nil)
        #expect(service.active?.id == second)
    }

    @Test("dismiss by extension id resolves the opener")
    func dismissByExtensionIDResolves() async {
        let service = ExtensionWebviewModalService()
        let requestID = service.open(request(entry: "a.html"))

        async let awaited = service.awaitClose(requestID: requestID)
        service.dismiss(extensionID: "ext")
        let result = await awaited

        #expect(result == nil)
        #expect(service.active == nil)
    }

    @Test("request id is namespaced so it cannot collide with picker modal ids")
    func requestIDIsNamespaced() {
        let service = ExtensionWebviewModalService()
        let requestID = service.open(request(extensionID: "ext"))
        #expect(requestID.hasPrefix("\(ExtensionWebviewModalService.requestIDPrefix):"))
        #expect(requestID != "ext:1")
    }
}

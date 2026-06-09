import Testing

@testable import Muxy

@Suite("ToastState", .serialized)
@MainActor
struct ToastStateTests {
    @Test("plain toasts keep the existing one-line message behavior")
    func plainToast() {
        let toast = ToastState.shared
        defer { toast.dismiss() }

        toast.show("Copied")

        #expect(toast.message == "Copied")
        #expect(toast.content == ToastContent(title: "Copied", body: nil, isActionable: false))
    }

    @Test("notification toasts store title body and action")
    func notificationToast() {
        let toast = ToastState.shared
        var didActivate = false
        defer { toast.dismiss() }

        toast.show(title: "Build finished", body: "All tests passed") {
            didActivate = true
        }
        toast.performAction()

        #expect(didActivate)
        #expect(toast.content == nil)
    }

    @Test("blank toast bodies are omitted")
    func blankBody() {
        let toast = ToastState.shared
        defer { toast.dismiss() }

        toast.show(title: "Done", body: "  \n  ")

        #expect(toast.content == ToastContent(title: "Done", body: nil, isActionable: false))
    }
}

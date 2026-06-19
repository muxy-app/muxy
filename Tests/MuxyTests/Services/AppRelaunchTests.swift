import AppKit
import Foundation
import Testing

@testable import Muxy

@Suite("AppRelaunch")
@MainActor
struct AppRelaunchTests {
    @Test("app bundles relaunch through nohup and open")
    func appBundleRelaunchUsesDetachedOpenRequest() {
        let bundleURL = URL(fileURLWithPath: "/Applications/Muxy Beta.app")
        let request = AppRelaunch.launchRequest(bundleURL: bundleURL, executableURL: nil, processID: 42)

        #expect(request.executableURL.path == "/usr/bin/nohup")
        #expect(request.arguments == [
            "/bin/sh",
            "-c",
            "while /bin/kill -0 \"$1\" 2>/dev/null; do /bin/sleep 0.1; done; /usr/bin/open \"$2\"",
            "muxy-relaunch",
            "42",
            "/Applications/Muxy Beta.app",
        ])
    }

    @Test("non app bundles relaunch the executable")
    func nonAppBundleRelaunchUsesExecutableRequest() {
        let bundleURL = URL(fileURLWithPath: "/tmp/MuxyPackageTests.xctest")
        let executableURL = URL(fileURLWithPath: "/tmp/Muxy")
        let request = AppRelaunch.launchRequest(bundleURL: bundleURL, executableURL: executableURL, processID: 7)

        #expect(request.executableURL.path == "/usr/bin/nohup")
        #expect(request.arguments == [
            "/bin/sh",
            "-c",
            "while /bin/kill -0 \"$1\" 2>/dev/null; do /bin/sleep 0.1; done; \"$2\"",
            "muxy-relaunch",
            "7",
            "/tmp/Muxy",
        ])
    }

    @Test("relaunch suppresses termination user-state persistence")
    func relaunchSuppressesTerminationUserStatePersistence() {
        AppRelaunch.resetForTesting()
        defer { AppRelaunch.resetForTesting() }

        let delegate = AppDelegate()
        var didPersist = false
        delegate.onTerminate = {
            didPersist = true
        }

        AppRelaunch.prepareForRelaunch()
        delegate.persistUserStateForTermination()

        #expect(!didPersist)
    }

    @Test("dismisses attached sheets so termination is not blocked")
    func dismissesAttachedSheetsBeforeTermination() {
        let parent = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 200, height: 200),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        let sheet = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 100, height: 100),
            styleMask: [.titled],
            backing: .buffered,
            defer: false
        )
        parent.beginSheet(sheet)
        pumpRunLoop(until: { parent.attachedSheet === sheet })
        #expect(parent.attachedSheet === sheet)

        AppRelaunch.dismissAttachedSheets(in: [parent])
        pumpRunLoop(until: { parent.attachedSheet == nil })

        #expect(parent.attachedSheet == nil)
    }

    private func pumpRunLoop(until condition: () -> Bool) {
        let deadline = Date().addingTimeInterval(2)
        while !condition(), Date() < deadline {
            RunLoop.current.run(mode: .default, before: Date().addingTimeInterval(0.01))
        }
    }
}

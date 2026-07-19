import Foundation
import Testing

@testable import Muxy

@Suite("HookHealthPresenter")
struct HookHealthPresenterTests {
    private let base = Date(timeIntervalSince1970: 1_700_000_000)

    @Test("dot color reflects install state")
    func dotColorReflectsState() {
        #expect(HookHealthPresenter.dot(for: health(.installed)) == .healthy)
        #expect(HookHealthPresenter.dot(for: health(.cliMissing)) == .warning)
        #expect(HookHealthPresenter.dot(for: health(.conflict("x"))) == .error)
        #expect(HookHealthPresenter.dot(for: health(.error("x"))) == .error)
        #expect(HookHealthPresenter.dot(for: health(.notInstalled)) == .idle)
    }

    @Test("an installed hook with a stale last event degrades to warning")
    func staleEventDegradesDot() {
        var value = health(.installed)
        value.lastEventAt = base.addingTimeInterval(-HookHealthPresenter.staleEventThreshold - 1)
        #expect(HookHealthPresenter.dot(for: value, now: base) == .warning)
    }

    @Test("an installed hook with a recent event stays healthy")
    func recentEventStaysHealthy() {
        var value = health(.installed)
        value.lastEventAt = base.addingTimeInterval(-60)
        #expect(HookHealthPresenter.dot(for: value, now: base) == .healthy)
    }

    @Test("healthy line shows last event time when present")
    func healthyLineShowsLastEvent() {
        var value = health(.installed)
        value.lastVerifiedAt = base
        value.lastEventAt = base.addingTimeInterval(-120)
        let line = HookHealthPresenter.statusLine(for: value, now: base)
        #expect(line == "Hook healthy · last event 2 min ago")
    }

    @Test("healthy line without events reports healthy")
    func healthyLineWithoutEvents() {
        var value = health(.installed)
        value.lastVerifiedAt = base
        #expect(HookHealthPresenter.statusLine(for: value, now: base) == "Hook healthy")
    }

    @Test("repaired install surfaces the repair message")
    func repairedLine() {
        var value = health(.installed)
        value.lastVerifiedAt = base
        value.lastRepairedAt = base
        let line = HookHealthPresenter.statusLine(for: value, now: base)
        #expect(line == "Config overwritten — repaired just now")
    }

    @Test("cli missing and conflict lines")
    func stateLines() {
        #expect(HookHealthPresenter.statusLine(for: health(.cliMissing), now: base) == "CLI not installed")
        #expect(HookHealthPresenter.statusLine(for: health(.conflict("both configs")), now: base) == "both configs")
        #expect(HookHealthPresenter.statusLine(for: health(.error("broke")), now: base) == "broke")
    }

    @Test("relative time formatting")
    func relativeTimeFormatting() {
        #expect(HookHealthPresenter.relative(from: base, now: base) == "just now")
        #expect(HookHealthPresenter.relative(from: base, now: base.addingTimeInterval(180)) == "3 min ago")
        #expect(HookHealthPresenter.relative(from: base, now: base.addingTimeInterval(7200)) == "2 hr ago")
        #expect(HookHealthPresenter.relative(from: base, now: base.addingTimeInterval(172_800)) == "2 d ago")
    }

    private func health(_ state: HookInstallState) -> HookHealth {
        HookHealth(installState: state)
    }
}

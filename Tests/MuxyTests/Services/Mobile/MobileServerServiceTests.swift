import Foundation
import Testing

@testable import Muxy

@Suite("Mobile server service")
@MainActor
struct MobileServerServiceTests {
    @Test("out-of-range scrollback cap is rejected and not persisted")
    func rejectsInvalidScrollbackCap() {
        let service = MobileServerService.shared
        let key = MobileServerService.scrollbackCapKey
        let originalServiceValue = service.scrollbackCapMB
        let originalStoredValue = UserDefaults.standard.object(forKey: key)
        defer {
            service.scrollbackCapMB = originalServiceValue
            if let originalStoredValue {
                UserDefaults.standard.set(originalStoredValue, forKey: key)
            } else {
                UserDefaults.standard.removeObject(forKey: key)
            }
        }
        UserDefaults.standard.set(originalServiceValue, forKey: key)

        service.scrollbackCapMB = 0
        #expect(service.scrollbackCapMB == originalServiceValue)
        #expect(UserDefaults.standard.integer(forKey: key) == originalServiceValue)

        service.scrollbackCapMB = 999
        #expect(service.scrollbackCapMB == originalServiceValue)
        #expect(UserDefaults.standard.integer(forKey: key) == originalServiceValue)
    }

    @Test("valid scrollback cap is persisted")
    func appliesValidScrollbackCap() {
        let service = MobileServerService.shared
        let key = MobileServerService.scrollbackCapKey
        let originalServiceValue = service.scrollbackCapMB
        let originalStoredValue = UserDefaults.standard.object(forKey: key)
        defer {
            service.scrollbackCapMB = originalServiceValue
            if let originalStoredValue {
                UserDefaults.standard.set(originalStoredValue, forKey: key)
            } else {
                UserDefaults.standard.removeObject(forKey: key)
            }
        }

        service.scrollbackCapMB = 16

        #expect(service.scrollbackCapMB == 16)
        #expect(UserDefaults.standard.integer(forKey: key) == 16)
    }
}
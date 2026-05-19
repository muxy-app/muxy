import Testing

@testable import Muxy

@Suite("PairingRequestCoordinator")
struct PairingRequestCoordinatorTests {
    @Test("validates token and device name lengths before queuing")
    func validatesRequestLengths() {
        #expect(PairingRequestCoordinator.isValidRequest(deviceName: "iPhone", token: String(repeating: "a", count: 8)))
        #expect(!PairingRequestCoordinator.isValidRequest(deviceName: "", token: String(repeating: "a", count: 8)))
        #expect(!PairingRequestCoordinator.isValidRequest(deviceName: String(repeating: "d", count: 129), token: String(repeating: "a", count: 8)))
        #expect(!PairingRequestCoordinator.isValidRequest(deviceName: "iPhone", token: String(repeating: "a", count: 7)))
        #expect(!PairingRequestCoordinator.isValidRequest(deviceName: "iPhone", token: String(repeating: "a", count: 257)))
    }
}

import Testing

@testable import Muxy

@Suite("PrivateNetworkGuard")
struct PrivateNetworkGuardTests {
    @Test("detects literal private, loopback, link-local, and metadata addresses")
    func detectsLiteralPrivateAddresses() {
        let privateHosts = [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "172.31.255.254",
            "192.168.1.1",
            "169.254.169.254",
            "::1",
            "fe80::1",
            "fd00:ec2::254",
        ]

        for host in privateHosts {
            #expect(PrivateNetworkGuard.isLiteralPrivateAddress(host))
        }
    }

    @Test("allows literal public addresses")
    func allowsLiteralPublicAddresses() {
        #expect(!PrivateNetworkGuard.isLiteralPrivateAddress("93.184.216.34"))
        #expect(!PrivateNetworkGuard.isLiteralPrivateAddress("2606:2800:220:1:248:1893:25c8:1946"))
    }

    @Test("rejects resolved localhost")
    func rejectsResolvedLocalhost() {
        #expect(!PrivateNetworkGuard.hostResolvesToPublicAddress("localhost"))
    }
}

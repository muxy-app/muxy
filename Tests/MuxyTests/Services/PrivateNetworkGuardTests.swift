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
            "100.64.0.1",
            "100.127.255.254",
            "224.0.0.1",
            "239.255.255.255",
            "240.0.0.1",
            "255.255.255.255",
            "192.0.2.1",
            "198.18.0.1",
            "198.51.100.1",
            "203.0.113.1",
            "::1",
            "fe80::1",
            "fd00:ec2::254",
            "ff00::1",
            "2001:db8::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "::ffff:169.254.169.254",
            "64:ff9b::a00:1",
            "64:ff9b:1:a00:1::",
            "2002:0a00:0001::",
            "2001::1",
            "2001:10::1",
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

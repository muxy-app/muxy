import Foundation
import Testing

@testable import Muxy

@Suite("MobilePairingURI")
struct MobilePairingURITests {
    @Test("includes host and port with required params")
    func basicURI() throws {
        let uri = try #require(MobilePairingURI.makeString(
            host: "Saeeds-MacBook-Pro.local",
            port: 4865,
            service: nil,
            label: nil
        ))
        let components = try #require(URLComponents(string: uri))
        #expect(components.scheme == "muxy")
        #expect(components.host == "pair")
        let items = components.queryItems ?? []
        #expect(items.contains(URLQueryItem(name: "host", value: "Saeeds-MacBook-Pro.local")))
        #expect(items.contains(URLQueryItem(name: "port", value: "4865")))
        #expect(!items.contains(where: { $0.name == "service" }))
        #expect(!items.contains(where: { $0.name == "label" }))
    }

    @Test("URL-encodes label with special characters")
    func encodesLabel() throws {
        let uri = try #require(MobilePairingURI.makeString(
            host: "host.local",
            port: 4865,
            service: "Saeeds-MacBook-Pro",
            label: "Saeed's Mac"
        ))
        #expect(uri.contains("label=Saeed"))
        let components = try #require(URLComponents(string: uri))
        let labelItem = components.queryItems?.first(where: { $0.name == "label" })
        #expect(labelItem?.value == "Saeed's Mac")
    }

    @Test("omits empty service and label")
    func omitsEmptyOptionals() throws {
        let uri = try #require(MobilePairingURI.makeString(
            host: "host.local",
            port: 4865,
            service: "",
            label: ""
        ))
        let components = try #require(URLComponents(string: uri))
        let names = (components.queryItems ?? []).map(\.name)
        #expect(!names.contains("service"))
        #expect(!names.contains("label"))
    }

    @Test("omits nil service and label")
    func omitsNilOptionals() throws {
        let uri = try #require(MobilePairingURI.makeString(
            host: "host.local",
            port: 4865,
            service: nil,
            label: nil
        ))
        let components = try #require(URLComponents(string: uri))
        let names = (components.queryItems ?? []).map(\.name)
        #expect(!names.contains("service"))
        #expect(!names.contains("label"))
    }

    @Test("rejects empty host")
    func rejectsEmptyHost() {
        #expect(MobilePairingURI.makeString(host: "  ", port: 4865, service: nil, label: nil) == nil)
    }
}

@Suite("MobilePairingService Tailscale detection")
struct MobilePairingServiceTailscaleTests {
    @Test("recognises CGNAT range as Tailscale")
    func recognisesCGNATRange() {
        #expect(MobilePairingService.isTailscaleAddress("100.64.0.1"))
        #expect(MobilePairingService.isTailscaleAddress("100.96.10.20"))
        #expect(MobilePairingService.isTailscaleAddress("100.127.255.254"))
    }

    @Test("rejects addresses outside CGNAT range")
    func rejectsNonCGNAT() {
        #expect(!MobilePairingService.isTailscaleAddress("100.63.0.1"))
        #expect(!MobilePairingService.isTailscaleAddress("100.128.0.1"))
        #expect(!MobilePairingService.isTailscaleAddress("192.168.1.1"))
        #expect(!MobilePairingService.isTailscaleAddress("10.0.0.1"))
    }

    @Test("rejects malformed addresses")
    func rejectsMalformed() {
        #expect(!MobilePairingService.isTailscaleAddress(""))
        #expect(!MobilePairingService.isTailscaleAddress("100.64.0"))
        #expect(!MobilePairingService.isTailscaleAddress("100.64.0.1.1"))
        #expect(!MobilePairingService.isTailscaleAddress("100.x.0.1"))
        #expect(!MobilePairingService.isTailscaleAddress("256.64.0.1"))
    }
}

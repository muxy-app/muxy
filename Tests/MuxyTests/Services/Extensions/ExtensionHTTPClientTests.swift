import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionHTTPClient", .serialized)
@MainActor
struct ExtensionHTTPClientTests {
    @Test("blocks localhost without prompting")
    func blocksLocalhost() async {
        await expectBlocked(url: "http://localhost:8080/health")
    }

    @Test("blocks loopback IPv4")
    func blocksLoopbackIPv4() async {
        await expectBlocked(url: "http://127.0.0.1/health")
    }

    @Test("blocks link-local metadata address")
    func blocksLinkLocal() async {
        await expectBlocked(url: "http://169.254.169.254/latest/meta-data/")
    }

    @Test("blocks private 192.168 address")
    func blocksPrivateClassC() async {
        await expectBlocked(url: "http://192.168.1.1/")
    }

    @Test("blocks private 10.x address")
    func blocksPrivateClassA() async {
        await expectBlocked(url: "http://10.0.0.5/")
    }

    @Test("blocks IPv6 loopback")
    func blocksIPv6Loopback() async {
        await expectBlocked(url: "http://[::1]/")
    }

    @Test("blocks .local hostnames")
    func blocksDotLocal() async {
        await expectBlocked(url: "http://printer.local/status")
    }

    @Test("blocks hosts that fail to resolve (fail closed)")
    func blocksUnresolvableHost() async {
        await expectBlocked(url: "https://muxy-nonexistent-host.invalid/x")
    }

    @Test("redirect guard rejects private redirect targets")
    func redirectGuardBlocksPrivateTargets() {
        #expect(HostSecurityPolicy.isBlocked("127.0.0.1"))
        #expect(HostSecurityPolicy.isBlocked("169.254.169.254"))
        #expect(HostSecurityPolicy.isBlocked("localhost"))
        #expect(HostSecurityPolicy.isBlocked("192.168.1.1"))
        #expect(HostSecurityPolicy.isBlocked("93.184.216.34") == false)
    }

    @Test("truncates responses larger than the byte cap")
    func truncatesLargeResponse() async throws {
        let oversized = Data(repeating: UInt8(ascii: "a"), count: ExtensionHTTPClient.maxResponseBytes + 1024)
        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!
            return (response, oversized)
        }
        defer { StubURLProtocol.handler = nil }

        let grantStore = preallowedStore(host: "93.184.216.34")
        let consent = ExtensionConsentService(grantStore: grantStore, auditLog: makeAuditLog())
        let request = HTTPRequest(url: "https://93.184.216.34/big", method: "GET", headers: nil, body: nil, timeoutMs: nil)
        let result = try await ExtensionHTTPClient.fetch(
            request: request,
            extensionID: "ext",
            session: stubSession(),
            consentService: consent
        )
        #expect(result.truncated == true)
        #expect(result.body.utf8.count <= ExtensionHTTPClient.maxResponseBytes)
    }

    @Test("rejects non-http schemes")
    func rejectsFileScheme() async {
        await expectInvalid(url: "file:///etc/passwd")
    }

    @Test("rejects unsupported methods")
    func rejectsUnsupportedMethod() async {
        let request = HTTPRequest(url: "https://example.com", method: "TRACE", headers: nil, body: nil, timeoutMs: nil)
        await #expect(throws: HTTPError.self) {
            _ = try await ExtensionHTTPClient.fetch(request: request, extensionID: "ext", session: stubSession())
        }
    }

    @Test("performs request and maps response after consent")
    func performsRequest() async throws {
        StubURLProtocol.handler = { request in
            #expect(request.httpMethod == "POST")
            #expect(request.value(forHTTPHeaderField: "X-Test") == "1")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 201,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, Data(#"{"ok":true}"#.utf8))
        }
        defer { StubURLProtocol.handler = nil }

        let grantStore = preallowedStore(host: "93.184.216.34")
        let consent = ExtensionConsentService(grantStore: grantStore, auditLog: makeAuditLog())

        let request = HTTPRequest(
            url: "https://93.184.216.34/api",
            method: "POST",
            headers: ["X-Test": "1"],
            body: #"{"a":1}"#,
            timeoutMs: 5000
        )
        let result = try await ExtensionHTTPClient.fetch(
            request: request,
            extensionID: "ext",
            session: stubSession(),
            consentService: consent
        )
        #expect(result.status == 201)
        #expect(result.headers["Content-Type"] == "application/json")
        #expect(result.body == #"{"ok":true}"#)
        #expect(result.truncated == false)
    }

    @Test("denied consent prevents the request")
    func deniedConsentFails() async {
        StubURLProtocol.handler = { _ in
            Issue.record("network must not be hit when consent is denied")
            let response = HTTPURLResponse(url: URL(string: "https://example.com")!, statusCode: 200, httpVersion: nil, headerFields: nil)!
            return (response, Data())
        }
        defer { StubURLProtocol.handler = nil }

        let grantStore = makeGrantStore()
        grantStore.add(ExtensionGrantRule(extensionID: "ext", verb: .httpFetch, match: .any, decision: .deny))
        let consent = ExtensionConsentService(grantStore: grantStore, auditLog: makeAuditLog())

        let request = HTTPRequest(url: "https://93.184.216.34", method: "GET", headers: nil, body: nil, timeoutMs: nil)
        await #expect(throws: HTTPError.self) {
            _ = try await ExtensionHTTPClient.fetch(
                request: request,
                extensionID: "ext",
                session: stubSession(),
                consentService: consent
            )
        }
    }

    private func expectBlocked(url: String) async {
        let request = HTTPRequest(url: url, method: "GET", headers: nil, body: nil, timeoutMs: nil)
        await #expect(throws: HTTPError.self) {
            _ = try await ExtensionHTTPClient.fetch(request: request, extensionID: "ext", session: stubSession())
        }
    }

    private func expectInvalid(url: String) async {
        let request = HTTPRequest(url: url, method: "GET", headers: nil, body: nil, timeoutMs: nil)
        await #expect(throws: HTTPError.self) {
            _ = try await ExtensionHTTPClient.fetch(request: request, extensionID: "ext", session: stubSession())
        }
    }

    private func stubSession() -> URLSession {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [StubURLProtocol.self]
        return URLSession(configuration: config)
    }

    private func preallowedStore(host: String) -> ExtensionGrantStore {
        let store = makeGrantStore()
        store.add(ExtensionGrantRule(extensionID: "ext", verb: .httpFetch, match: .hostEquals(host), decision: .allow))
        return store
    }

    private func makeGrantStore() -> ExtensionGrantStore {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-http-grant-\(UUID().uuidString).json")
        return ExtensionGrantStore(fileURL: url)
    }

    private func makeAuditLog() -> ExtensionAuditLog {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-http-audit-\(UUID().uuidString).log")
        return ExtensionAuditLog(fileURL: url)
    }
}

private final class StubURLProtocol: URLProtocol {
    nonisolated(unsafe) static var handler: ((URLRequest) -> (HTTPURLResponse, Data))?

    override class func canInit(with _: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let handler = StubURLProtocol.handler else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }
        let (response, data) = handler(request)
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: data)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

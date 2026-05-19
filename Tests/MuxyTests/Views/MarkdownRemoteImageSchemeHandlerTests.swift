import Foundation
import Testing

@testable import Muxy

@Suite("MarkdownRemoteImageSchemeHandler")
struct MarkdownRemoteImageSchemeHandlerTests {
    @Test("decode rejects literal private and metadata hosts")
    func decodeRejectsLiteralPrivateHosts() throws {
        let urls = [
            try Self.schemeURL(for: "https://127.0.0.1/image.png"),
            try Self.schemeURL(for: "https://10.0.0.5/image.png"),
            try Self.schemeURL(for: "https://192.168.1.20/image.png"),
            try Self.schemeURL(for: "https://169.254.169.254/latest/meta-data"),
            try Self.schemeURL(for: "https://[fd00:ec2::254]/latest/meta-data"),
        ]

        for url in urls {
            #expect(MarkdownRemoteImageSchemeHandler.decodeRemoteURL(from: url) == nil)
        }
    }

    @Test("redirect rejects hosts that resolve to private addresses")
    func redirectRejectsPrivateResolvedHost() throws {
        let request = URLRequest(url: try #require(URL(string: "https://localhost/image.png")))

        #expect(MarkdownRemoteImageSchemeHandler.redirectRequestIfAllowed(request) == nil)
    }

    @Test("redirect allows literal public HTTPS address")
    func redirectAllowsLiteralPublicAddress() throws {
        let request = URLRequest(url: try #require(URL(string: "https://93.184.216.34/image.png")))

        #expect(MarkdownRemoteImageSchemeHandler.redirectRequestIfAllowed(request)?.url == request.url)
    }

    private static func schemeURL(for remoteURL: String) throws -> URL {
        let token = Data(remoteURL.utf8).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
        return try #require(URL(string: "\(MarkdownRemoteImageSchemeHandler.scheme)://image/\(token)"))
    }
}

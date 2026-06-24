import Foundation
import Testing
@testable import Muxy

@Suite("TerminalOpenURLParser")
struct TerminalOpenURLParserTests {
    @Test func parsesStandardHTTPS() throws {
        let url = try #require(TerminalOpenURLParser.url(from: "https://example.com/a?b=1"))
        #expect(url.scheme == "https")
        #expect(url.host == "example.com")
    }

    @Test func rejectsEmpty() {
        #expect(TerminalOpenURLParser.url(from: "") == nil)
    }

    @Test func parsesFileURL() throws {
        let url = try #require(TerminalOpenURLParser.url(from: "file:///tmp/x.md"))
        #expect(url.isFileURL)
    }
}

@Suite("TerminalOSC8LinkState")
struct TerminalOSC8LinkStateTests {
    @Test func hoverWithURLSetsStickyAndUnderCursor() throws {
        var state = TerminalOSC8LinkState()
        state.applyHover(urlString: "https://example.com/x", commandHeld: false)

        #expect(state.hasLinkUnderCursor)
        #expect(state.stickyURL == try #require(URL(string: "https://example.com/x")))
        #expect(state.shouldShowLinkCursor)
        #expect(state.urlToOpenOnCommandClick() == state.stickyURL)
    }

    @Test func emptyHoverWithoutCommandClearsSticky() throws {
        var state = TerminalOSC8LinkState()
        state.applyHover(urlString: "https://example.com/x", commandHeld: false)
        state.applyHover(urlString: nil, commandHeld: false)

        #expect(!state.hasLinkUnderCursor)
        #expect(state.stickyURL == nil)
        #expect(!state.shouldShowLinkCursor)
        #expect(state.urlToOpenOnCommandClick() == nil)
    }

    @Test func emptyHoverWithCommandKeepsStickyForCmdClick() throws {
        var state = TerminalOSC8LinkState()
        state.applyHover(urlString: "https://example.com/x", commandHeld: false)
        state.applyHover(urlString: "", commandHeld: true)

        #expect(!state.hasLinkUnderCursor)
        #expect(state.stickyURL == try #require(URL(string: "https://example.com/x")))
        #expect(state.shouldShowLinkCursor)
        #expect(state.urlToOpenOnCommandClick() == try #require(URL(string: "https://example.com/x")))
    }

    @Test func emptyStringHoverWithCommandPreservesSticky() throws {
        var state = TerminalOSC8LinkState()
        state.applyHover(urlString: "https://example.com/docs", commandHeld: false)
        state.applyHover(urlString: "", commandHeld: true)

        #expect(state.urlToOpenOnCommandClick()?.absoluteString == "https://example.com/docs")
    }

    @Test func invalidURLDoesNotSetSticky() {
        var state = TerminalOSC8LinkState()
        state.applyHover(urlString: "not a url with spaces", commandHeld: false)

        #expect(!state.hasLinkUnderCursor)
        #expect(state.stickyURL == nil)
    }

    @Test func replacingHoverUpdatesStickyURL() throws {
        var state = TerminalOSC8LinkState()
        state.applyHover(urlString: "https://a.example", commandHeld: false)
        state.applyHover(urlString: "https://b.example", commandHeld: false)

        #expect(state.stickyURL == try #require(URL(string: "https://b.example")))
    }

    @Test func leaveLinkThenCmdHeldDoesNotResurrectSticky() {
        var state = TerminalOSC8LinkState()
        state.applyHover(urlString: "https://example.com/x", commandHeld: false)
        state.applyHover(urlString: nil, commandHeld: false)
        state.applyHover(urlString: nil, commandHeld: true)

        #expect(state.stickyURL == nil)
        #expect(state.urlToOpenOnCommandClick() == nil)
    }
}

@Suite("TerminalBridge OSC8 open routing")
@MainActor
struct TerminalBridgeOSC8OpenRoutingTests {
    @Test func externalHTTPSIsOpenable() throws {
        let url = try #require(URL(string: "https://example.com/path?q=1"))
        #expect(TerminalBridge.isExternalLink(url))
    }

    @Test func externalHTTPIsOpenable() throws {
        let url = try #require(URL(string: "http://localhost:8080/"))
        #expect(TerminalBridge.isExternalLink(url))
    }

    @Test func fileURLIsNotExternalLink() throws {
        let url = try #require(URL(string: "file:///tmp/readme.md"))
        #expect(!TerminalBridge.isExternalLink(url))
    }

    @Test func localPathTokenIsNotExternalLink() throws {
        let url = try #require(URL(string: "src/main.swift:12"))
        #expect(!TerminalBridge.isExternalLink(url))
    }

    @Test func stickyCmdClickURLMatchesHoverBeforeCommandClear() throws {
        var state = TerminalOSC8LinkState()
        let hovered = try #require(URL(string: "https://example.com/link"))
        state.applyHover(urlString: hovered.absoluteString, commandHeld: false)
        state.applyHover(urlString: nil, commandHeld: true)

        let toOpen = try #require(state.urlToOpenOnCommandClick())
        #expect(TerminalBridge.isExternalLink(toOpen))
        #expect(toOpen == hovered)
    }
}

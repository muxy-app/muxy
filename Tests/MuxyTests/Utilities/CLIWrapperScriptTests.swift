import Foundation
import Testing

@testable import Muxy

@Suite("CLIWrapperScript")
struct CLIWrapperScriptTests {
    @Test("wrapper execs the bundled script rather than embedding the CLI body")
    func wrapperExecsBundledScript() {
        let wrapper = CLIWrapperScript.contents(installedAppPath: "/Applications/Muxy.app")
        #expect(wrapper.hasPrefix("#!/bin/bash"))
        #expect(wrapper.contains("exec \"$SCRIPT\" \"$@\""))
        #expect(wrapper.contains(CLIWrapperScript.bundledScriptRelativePath))
        #expect(!wrapper.contains("send_command"))
    }

    @Test("wrapper resolves the app by bundle id so it survives moves")
    func wrapperResolvesByBundleID() {
        let wrapper = CLIWrapperScript.contents(installedAppPath: "/Applications/Muxy.app")
        #expect(wrapper.contains("kMDItemCFBundleIdentifier == \(CLIWrapperScript.bundleIdentifier)"))
        #expect(wrapper.contains("mdfind"))
    }

    @Test("wrapper honors MUXY_APP_PATH and falls back to standard locations")
    func wrapperHonorsOverrideAndFallbacks() {
        let wrapper = CLIWrapperScript.contents(installedAppPath: "/Applications/Muxy.app")
        #expect(wrapper.contains("${MUXY_APP_PATH:-}"))
        #expect(wrapper.contains("/Applications/Muxy.app"))
        #expect(wrapper.contains("$HOME/Applications/Muxy.app"))
    }

    @Test("captured app path with spaces is shell-quoted")
    func capturedAppPathIsQuoted() {
        let wrapper = CLIWrapperScript.contents(installedAppPath: "/Users/a/My Apps/Muxy.app")
        #expect(wrapper.contains("'/Users/a/My Apps/Muxy.app'"))
    }
}

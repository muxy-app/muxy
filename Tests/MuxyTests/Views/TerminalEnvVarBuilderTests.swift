import Foundation
import Testing

@testable import Muxy

@MainActor
@Suite("TerminalEnvVarBuilder")
struct TerminalEnvVarBuilderTests {
    @Test("prepends Muxy bin to PATH when Pi wrapper is installed")
    func prependsMuxyBinToPATH() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("TerminalEnvVarBuilderTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let appSupport = root.appendingPathComponent("Muxy", isDirectory: true)
        let bin = MuxyAgentBin.directory(appSupportDirectory: appSupport)
        try FileManager.default.createDirectory(at: bin, withIntermediateDirectories: true)

        let wrapper = MuxyAgentBin.wrapperURL(appSupportDirectory: appSupport)
        try Data("#!/usr/bin/env bash\n".utf8).write(to: wrapper)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.executable],
            ofItemAtPath: wrapper.path
        )

        let path = TerminalEnvVarBuilder.terminalPATH(
            appSupportDirectory: appSupport,
            inheritedPath: "/usr/bin:/bin"
        )

        #expect(path == "\(bin.path):/usr/bin:/bin")
    }

    @Test("does not override PATH when Pi wrapper is absent")
    func leavesPATHUnsetWithoutWrapper() {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("TerminalEnvVarBuilderTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let appSupport = root.appendingPathComponent("Muxy", isDirectory: true)

        let path = TerminalEnvVarBuilder.terminalPATH(
            appSupportDirectory: appSupport,
            inheritedPath: "/usr/bin:/bin"
        )

        #expect(path == nil)
    }

    @Test("build includes PATH when Pi wrapper is installed")
    func buildIncludesPATH() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("TerminalEnvVarBuilderTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let appSupport = root.appendingPathComponent("Muxy", isDirectory: true)
        let bin = MuxyAgentBin.directory(appSupportDirectory: appSupport)
        try FileManager.default.createDirectory(at: bin, withIntermediateDirectories: true)

        let wrapper = MuxyAgentBin.wrapperURL(appSupportDirectory: appSupport)
        try Data("#!/usr/bin/env bash\n".utf8).write(to: wrapper)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.executable],
            ofItemAtPath: wrapper.path
        )

        let vars = TerminalEnvVarBuilder.build(
            paneID: UUID(),
            worktreeKey: WorktreeKey(projectID: UUID(), worktreeID: UUID()),
            appSupportDirectory: appSupport,
            inheritedPath: "/usr/bin"
        )

        let pathPair = vars.first { $0.key == "PATH" }
        #expect(pathPair?.value == "\(bin.path):/usr/bin")
    }
}

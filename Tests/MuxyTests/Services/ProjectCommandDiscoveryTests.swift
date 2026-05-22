import Foundation
import Testing

@testable import Muxy

@Suite("ProjectCommandDiscovery")
struct ProjectCommandDiscoveryTests {
    @Test("discovers npm scripts")
    func discoversNPMScripts() throws {
        let directory = try temporaryDirectory()
        try #"{"scripts":{"dev":"vite","test":"vitest"}}"#.write(
            to: directory.appendingPathComponent("package.json"),
            atomically: true,
            encoding: .utf8
        )

        let commands = NPMProjectCommandProvider().commands(projectPath: directory.path)

        #expect(commands.map(\.name) == ["dev", "test"])
        #expect(commands.first?.command == "npm run dev")
        #expect(commands.first?.source == .npm)
    }

    @Test("discovers composer scripts")
    func discoversComposerScripts() throws {
        let directory = try temporaryDirectory()
        try #"{"scripts":{"analyse":"phpstan","test":["phpunit"]}}"#.write(
            to: directory.appendingPathComponent("composer.json"),
            atomically: true,
            encoding: .utf8
        )

        let commands = ComposerProjectCommandProvider().commands(projectPath: directory.path)

        #expect(commands.map(\.name) == ["analyse", "test"])
        #expect(commands.first?.command == "composer run-script analyse")
        #expect(commands.first?.source == .composer)
    }

    private func temporaryDirectory() throws -> URL {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }
}

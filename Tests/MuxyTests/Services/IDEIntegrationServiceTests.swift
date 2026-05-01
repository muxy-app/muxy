import Foundation
import Testing

@testable import Muxy

@Suite("IDEIntegrationService")
@MainActor
struct IDEIntegrationServiceTests {
    @Test("resolveDefaultIDE returns Finder when Finder was the remembered launcher target")
    func resolveDefaultIDEReturnsFinderWhenFinderWasRemembered() {
        let vscode = IDEIntegrationService.IDEApplication(
            bundleIdentifier: "com.microsoft.VSCode",
            displayName: "VS Code",
            appURL: URL(fileURLWithPath: "/Applications/Visual Studio Code.app"),
            symbolName: "chevron.left.forwardslash.chevron.right",
            rank: 10,
            group: .editor
        )

        let resolved = IDEIntegrationService.resolveDefaultIDE(
            installedApps: [vscode],
            selectedBundleIdentifier: IDEIntegrationService.finderBundleIdentifier
        )

        #expect(resolved?.bundleIdentifier == IDEIntegrationService.finderBundleIdentifier)
        #expect(resolved?.displayName == "Finder")
    }

    @Test("resolveDefaultIDE prefers remembered selection when installed")
    func resolveDefaultIDEPrefersRememberedSelection() {
        let vscode = IDEIntegrationService.IDEApplication(
            bundleIdentifier: "com.microsoft.VSCode",
            displayName: "VS Code",
            appURL: URL(fileURLWithPath: "/Applications/Visual Studio Code.app"),
            symbolName: "chevron.left.forwardslash.chevron.right",
            rank: 10,
            group: .editor
        )
        let zed = IDEIntegrationService.IDEApplication(
            bundleIdentifier: "dev.zed.Zed",
            displayName: "Zed",
            appURL: URL(fileURLWithPath: "/Applications/Zed.app"),
            symbolName: "bolt.horizontal",
            rank: 13,
            group: .editor
        )

        let resolved = IDEIntegrationService.resolveDefaultIDE(
            installedApps: [vscode, zed],
            selectedBundleIdentifier: zed.bundleIdentifier
        )

        #expect(resolved?.bundleIdentifier == zed.bundleIdentifier)
    }

    @Test("launchCommands uses vscode CLI goto strategy when available")
    func launchCommandsUsesVSCodeCLIGotoStrategyWhenAvailable() {
        let ide = IDEIntegrationService.IDEApplication(
            bundleIdentifier: "com.microsoft.VSCode",
            displayName: "VS Code",
            appURL: URL(fileURLWithPath: "/Applications/Visual Studio Code.app"),
            symbolName: "chevron.left.forwardslash.chevron.right",
            rank: 10,
            group: .editor
        )
        let location = IDEIntegrationService.EditorLocation(
            filePath: "/tmp/repo/Sources/App.swift",
            line: 12,
            column: 7
        )

        let commands = IDEIntegrationService.launchCommands(
            for: ide,
            projectPath: "/tmp/repo",
            editorLocation: location,
            availableCLICommands: ["code": "/usr/local/bin/code"]
        )

        #expect(commands == [
            .init(
                executablePath: "/usr/local/bin/code",
                arguments: ["/tmp/repo", "--goto", "/tmp/repo/Sources/App.swift:12:7"]
            ),
        ])
    }

    @Test("launchCommands uses zed CLI when available")
    func launchCommandsUsesZedCLIWhenAvailable() {
        let ide = IDEIntegrationService.IDEApplication(
            bundleIdentifier: "dev.zed.Zed",
            displayName: "Zed",
            appURL: URL(fileURLWithPath: "/Applications/Zed.app"),
            symbolName: "bolt.horizontal",
            rank: 14,
            group: .editor
        )
        let location = IDEIntegrationService.EditorLocation(
            filePath: "/tmp/repo/Sources/App.swift",
            line: 12,
            column: 7
        )

        let commands = IDEIntegrationService.launchCommands(
            for: ide,
            projectPath: "/tmp/repo",
            editorLocation: location,
            availableCLICommands: ["zed": "/usr/local/bin/zed"]
        )

        #expect(commands == [
            .init(
                executablePath: "/usr/local/bin/zed",
                arguments: ["/tmp/repo", "/tmp/repo/Sources/App.swift:12:7"]
            ),
        ])
    }

    @Test("launchCommands uses generic opening for JetBrains even when launcher is available")
    func launchCommandsUsesGenericOpeningForJetBrainsEvenWhenLauncherIsAvailable() {
        let ide = IDEIntegrationService.IDEApplication(
            bundleIdentifier: "com.jetbrains.PhpStorm",
            displayName: "PhpStorm",
            appURL: URL(fileURLWithPath: "/Applications/PhpStorm.app"),
            symbolName: "chevron.left.forwardslash.chevron.right",
            rank: 17,
            group: .editor
        )
        let location = IDEIntegrationService.EditorLocation(
            filePath: "/tmp/repo/Sources/App.swift",
            line: 12,
            column: 7
        )

        let commands = IDEIntegrationService.launchCommands(
            for: ide,
            projectPath: "/tmp/repo",
            editorLocation: location,
            availableCLICommands: ["phpstorm": "/usr/local/bin/phpstorm"]
        )

        #expect(commands == [
            .init(
                executablePath: "/usr/bin/open",
                arguments: ["-a", "/Applications/PhpStorm.app", "/tmp/repo", "/tmp/repo/Sources/App.swift"]
            ),
        ])
    }

    @Test("launchCommands falls back to generic project and file opening")
    func launchCommandsFallsBackToGenericProjectAndFileOpening() {
        let ide = IDEIntegrationService.IDEApplication(
            bundleIdentifier: "com.jetbrains.PhpStorm",
            displayName: "PhpStorm",
            appURL: URL(fileURLWithPath: "/Applications/PhpStorm.app"),
            symbolName: "chevron.left.forwardslash.chevron.right",
            rank: 19,
            group: .editor
        )
        let location = IDEIntegrationService.EditorLocation(
            filePath: "/tmp/repo/Sources/App.swift",
            line: 12,
            column: 7
        )

        let commands = IDEIntegrationService.launchCommands(
            for: ide,
            projectPath: "/tmp/repo",
            editorLocation: location,
            availableCLICommands: [:]
        )

        #expect(commands == [
            .init(
                executablePath: "/usr/bin/open",
                arguments: ["-a", "/Applications/PhpStorm.app", "/tmp/repo", "/tmp/repo/Sources/App.swift"]
            ),
        ])
    }

    @Test("openTargetArguments includes project and focused file once")
    func openTargetArgumentsIncludesProjectAndFocusedFileOnce() {
        let arguments = IDEIntegrationService.openTargetArguments(
            projectPath: "/tmp/repo",
            filePath: "/tmp/repo/Sources/App.swift"
        )

        #expect(arguments == ["/tmp/repo", "/tmp/repo/Sources/App.swift"])
    }

    @Test("openTargetArguments omits duplicate or empty focused file")
    func openTargetArgumentsOmitsDuplicateOrEmptyFocusedFile() {
        #expect(IDEIntegrationService.openTargetArguments(projectPath: "/tmp/repo", filePath: "/tmp/repo") == ["/tmp/repo"])
        #expect(IDEIntegrationService.openTargetArguments(projectPath: "/tmp/repo", filePath: nil) == ["/tmp/repo"])
        #expect(IDEIntegrationService.openTargetArguments(projectPath: "/tmp/repo", filePath: "") == ["/tmp/repo"])
    }

    @Test("resolveDefaultIDE falls back to first installed IDE when selection is missing")
    func resolveDefaultIDEFallsBackToFirstInstalled() {
        let vscode = IDEIntegrationService.IDEApplication(
            bundleIdentifier: "com.microsoft.VSCode",
            displayName: "VS Code",
            appURL: URL(fileURLWithPath: "/Applications/Visual Studio Code.app"),
            symbolName: "chevron.left.forwardslash.chevron.right",
            rank: 10,
            group: .editor
        )
        let zed = IDEIntegrationService.IDEApplication(
            bundleIdentifier: "dev.zed.Zed",
            displayName: "Zed",
            appURL: URL(fileURLWithPath: "/Applications/Zed.app"),
            symbolName: "bolt.horizontal",
            rank: 13,
            group: .editor
        )

        let resolved = IDEIntegrationService.resolveDefaultIDE(
            installedApps: [vscode, zed],
            selectedBundleIdentifier: "com.example.missing"
        )

        #expect(resolved?.bundleIdentifier == vscode.bundleIdentifier)
    }

    @Test("classifies JetBrains IDEs automatically")
    func classifiesJetBrainsIDEsAutomatically() {
        let metadata = IDEIntegrationService.AppMetadata(
            bundleIdentifier: "com.jetbrains.PhpStorm",
            displayName: "PhpStorm",
            executableName: "phpstorm",
            category: "public.app-category.developer-tools",
            appURL: URL(fileURLWithPath: "/Applications/PhpStorm.app")
        )

        let app = IDEIntegrationService.ideApplication(from: metadata)

        #expect(app != nil)
        #expect(app?.displayName == "PhpStorm")
    }

    @Test("classifies developer tools by editor-like names")
    func classifiesDeveloperToolsByEditorLikeNames() {
        let metadata = IDEIntegrationService.AppMetadata(
            bundleIdentifier: "com.example.code-editor",
            displayName: "Acme Code Editor",
            executableName: "AcmeCodeEditor",
            category: "public.app-category.developer-tools",
            appURL: URL(fileURLWithPath: "/Applications/Acme Code Editor.app")
        )

        let app = IDEIntegrationService.ideApplication(from: metadata)

        #expect(app != nil)
        #expect(app?.group == .editor)
    }

    @Test("does not classify AI developer tools by generic keywords without a curated desktop bundle")
    func doesNotClassifyAIDeveloperToolsByGenericKeywordsWithoutCuratedDesktopBundle() {
        let metadata = IDEIntegrationService.AppMetadata(
            bundleIdentifier: "com.example.codex-wrapper",
            displayName: "Codex Wrapper",
            executableName: "codex",
            category: "public.app-category.developer-tools",
            appURL: URL(fileURLWithPath: "/Applications/Codex Wrapper.app")
        )

        #expect(IDEIntegrationService.ideApplication(from: metadata) == nil)
    }

    @Test("does not classify Jcode-like wrappers by generic code fallback")
    func doesNotClassifyJcodeLikeWrappersByGenericCodeFallback() {
        let metadata = IDEIntegrationService.AppMetadata(
            bundleIdentifier: "com.example.jcode-wrapper",
            displayName: "Jcode Wrapper",
            executableName: "jcode",
            category: "public.app-category.developer-tools",
            appURL: URL(fileURLWithPath: "/Applications/Jcode Wrapper.app")
        )

        #expect(IDEIntegrationService.ideApplication(from: metadata) == nil)
    }

    @Test("does not classify Claude Code style wrappers by generic code fallback")
    func doesNotClassifyClaudeCodeStyleWrappersByGenericCodeFallback() {
        let metadata = IDEIntegrationService.AppMetadata(
            bundleIdentifier: "com.example.claude-code-wrapper",
            displayName: "Claude Code",
            executableName: "claude-code",
            category: "public.app-category.developer-tools",
            appURL: URL(fileURLWithPath: "/Applications/Claude Code.app")
        )

        #expect(IDEIntegrationService.ideApplication(from: metadata) == nil)
    }

    @Test("classifies Emacs by curated bundle identifier")
    func classifiesEmacsByCuratedBundleIdentifier() {
        let metadata = IDEIntegrationService.AppMetadata(
            bundleIdentifier: "org.gnu.Emacs",
            displayName: "Emacs",
            executableName: "Emacs",
            category: nil,
            appURL: URL(fileURLWithPath: "/Applications/Emacs.app")
        )

        let app = IDEIntegrationService.ideApplication(from: metadata)

        #expect(app != nil)
        #expect(app?.displayName == "Emacs")
        #expect(app?.group == .editor)
    }

    @Test("classifies Emacs forks by keyword when bundle identifier is unknown")
    func classifiesEmacsForksByKeywordWhenBundleIdentifierIsUnknown() {
        let metadata = IDEIntegrationService.AppMetadata(
            bundleIdentifier: "org.example.emacs-mac",
            displayName: "Emacs",
            executableName: "Emacs",
            category: nil,
            appURL: URL(fileURLWithPath: "/Applications/Emacs.app")
        )

        let app = IDEIntegrationService.ideApplication(from: metadata)

        #expect(app != nil)
        #expect(app?.group == .editor)
    }

    @Test("classifies Aquamacs by curated bundle identifier")
    func classifiesAquamacsByCuratedBundleIdentifier() {
        let metadata = IDEIntegrationService.AppMetadata(
            bundleIdentifier: "org.aquamacs.Aquamacs",
            displayName: "Aquamacs",
            executableName: "Aquamacs",
            category: nil,
            appURL: URL(fileURLWithPath: "/Applications/Aquamacs.app")
        )

        let app = IDEIntegrationService.ideApplication(from: metadata)

        #expect(app != nil)
        #expect(app?.displayName == "Aquamacs")
        #expect(app?.group == .editor)
    }

    @Test("does not classify JetBrains Toolbox as an IDE target")
    func doesNotClassifyJetBrainsToolbox() {
        let metadata = IDEIntegrationService.AppMetadata(
            bundleIdentifier: "com.jetbrains.toolbox",
            displayName: "JetBrains Toolbox",
            executableName: "jetbrains-toolbox",
            category: "public.app-category.developer-tools",
            appURL: URL(fileURLWithPath: "/Applications/JetBrains Toolbox.app")
        )

        #expect(IDEIntegrationService.ideApplication(from: metadata) == nil)
    }

    @Test("sort prioritizes IDEs before AI companion apps and honors rank within a group")
    func sortPrioritizesIDEsBeforeAICompanionAppsAndHonorsRankWithinGroup() {
        let apps = [
            IDEIntegrationService.IDEApplication(
                bundleIdentifier: "com.jetbrains.air",
                displayName: "Air",
                appURL: URL(fileURLWithPath: "/Applications/Air.app"),
                symbolName: "sparkles",
                rank: 84,
                group: .otherTool
            ),
            IDEIntegrationService.IDEApplication(
                bundleIdentifier: "com.jetbrains.PhpStorm",
                displayName: "PhpStorm",
                appURL: URL(fileURLWithPath: "/Applications/PhpStorm.app"),
                symbolName: "chevron.left.forwardslash.chevron.right",
                rank: 17,
            group: .editor
            ),
            IDEIntegrationService.IDEApplication(
                bundleIdentifier: "com.microsoft.VSCode",
                displayName: "VS Code",
                appURL: URL(fileURLWithPath: "/Applications/Visual Studio Code.app"),
                symbolName: "chevron.left.forwardslash.chevron.right",
                rank: 10,
            group: .editor
            ),
            IDEIntegrationService.IDEApplication(
                bundleIdentifier: "com.google.antigravity",
                displayName: "Antigravity",
                appURL: URL(fileURLWithPath: "/Applications/Antigravity.app"),
                symbolName: "sparkles",
                rank: 82,
                group: .otherTool
            ),
        ]

        let sorted = apps.sorted(by: IDEIntegrationService.compareInstalledApps)

        #expect(sorted.map(\.displayName) == ["VS Code", "PhpStorm", "Antigravity", "Air"])
    }
}

import AppKit
import Foundation

enum HelpLinks {
    static let repoURL = url("https://github.com/muxy-app/muxy")
    static let docsURL = url("https://muxy.app/docs")
    static let mobileRepoURL = url("https://github.com/muxy-app/mobile")
    static let discordURL = url("https://discord.gg/4eMXAmJQ2n")
    static let issuesURL = url("https://github.com/muxy-app/muxy/issues")
    static let v2BetaReleasesURL = url("https://github.com/muxy-app/muxy/releases?q=v2.0.0-beta-")

    private static func url(_ string: String) -> URL {
        URL(string: string) ?? URL(fileURLWithPath: "/")
    }

    static func openRepo() {
        NSWorkspace.shared.open(repoURL)
    }

    static func openDocs() {
        NSWorkspace.shared.open(docsURL)
    }

    static func openMobileRepo() {
        NSWorkspace.shared.open(mobileRepoURL)
    }

    static func openDiscord() {
        NSWorkspace.shared.open(discordURL)
    }

    static func openIssues() {
        NSWorkspace.shared.open(issuesURL)
    }
}

import Testing
import CoreGraphics

@testable import Muxy

@Suite("SidebarLayout")
@MainActor
struct SidebarLayoutTests {
    @Test("collapsed hidden sidebar has zero width and is hidden")
    func collapsedHiddenSidebar() {
        #expect(SidebarLayout.resolvedWidth(
            expanded: false,
            collapsedStyle: .hidden,
            expandedStyle: .wide
        ) == 0)
        #expect(SidebarLayout.isHidden(expanded: false, collapsedStyle: .hidden))
        #expect(!SidebarLayout.isWide(expanded: false, expandedStyle: .wide))
    }

    @Test("collapsed icons sidebar uses icon rail width")
    func collapsedIconsSidebar() {
        #expect(SidebarLayout.resolvedWidth(
            expanded: false,
            collapsedStyle: .icons,
            expandedStyle: .wide
        ) == SidebarLayout.collapsedWidth)
        #expect(!SidebarLayout.isHidden(expanded: false, collapsedStyle: .icons))
        #expect(!SidebarLayout.isWide(expanded: false, expandedStyle: .wide))
    }

    @Test("expanded icons sidebar remains an icon rail without becoming hidden")
    func expandedIconsSidebar() {
        #expect(SidebarLayout.resolvedWidth(
            expanded: true,
            collapsedStyle: .hidden,
            expandedStyle: .icons
        ) == SidebarLayout.collapsedWidth)
        #expect(!SidebarLayout.isHidden(expanded: true, collapsedStyle: .hidden))
        #expect(!SidebarLayout.isWide(expanded: true, expandedStyle: .icons))
    }

    @Test("expanded wide sidebar uses full sidebar width")
    func expandedWideSidebar() {
        #expect(SidebarLayout.resolvedWidth(
            expanded: true,
            collapsedStyle: .hidden,
            expandedStyle: .wide
        ) == SidebarLayout.expandedWidth)
        #expect(!SidebarLayout.isHidden(expanded: true, collapsedStyle: .hidden))
        #expect(SidebarLayout.isWide(expanded: true, expandedStyle: .wide))
    }
}

@Suite("MainWindowLayout")
struct MainWindowLayoutTests {
    @Test("collapsed icon sidebar keeps its own rail width")
    func collapsedIconSidebarKeepsRailWidth() {
        #expect(MainWindowLayout.leftNavigationWidth(sidebarWidth: 44) == 44)
    }

    @Test("wide sidebar owns the title bar height instead of sitting below tab strip")
    func wideSidebarExtendsThroughTitleBar() {
        #expect(MainWindowLayout.leftNavigationWidth(sidebarWidth: 220) == 220)
        #expect(MainWindowLayout.titleBarNavigationOverlayWidth(
            leftNavigationWidth: 220,
            titleBarNavigationWidth: 127,
            isFullScreen: false
        ) == 220)
        #expect(MainWindowLayout.mainTitleBarLeadingInset(
            leftNavigationWidth: 220,
            titleBarNavigationOverlayWidth: 220,
            isFullScreen: false
        ) == 0)
    }

    @Test("narrow visible sidebar reserves only overflow in main title bar")
    func narrowSidebarReservesOverflowInTitleBar() {
        #expect(MainWindowLayout.titleBarNavigationOverlayWidth(
            leftNavigationWidth: 44,
            titleBarNavigationWidth: 127,
            isFullScreen: false
        ) == 127)
        #expect(MainWindowLayout.mainTitleBarLeadingInset(
            leftNavigationWidth: 44,
            titleBarNavigationOverlayWidth: 127,
            isFullScreen: false
        ) == 83)
    }

    @Test("hidden sidebar keeps full title bar navigation inset")
    func hiddenSidebarLeavesNavigationInTitleBar() {
        #expect(MainWindowLayout.leftNavigationWidth(sidebarWidth: 0) == 0)
        #expect(MainWindowLayout.titleBarNavigationOverlayWidth(
            leftNavigationWidth: 0,
            titleBarNavigationWidth: 127,
            isFullScreen: false
        ) == 127)
        #expect(MainWindowLayout.mainTitleBarLeadingInset(
            leftNavigationWidth: 0,
            titleBarNavigationOverlayWidth: 127,
            isFullScreen: false
        ) == 127)
    }

    @Test("full screen suppresses title bar navigation overlay")
    func fullScreenSuppressesTitleBarOverlay() {
        #expect(MainWindowLayout.titleBarNavigationOverlayWidth(
            leftNavigationWidth: 44,
            titleBarNavigationWidth: 127,
            isFullScreen: true
        ) == 0)
        #expect(MainWindowLayout.mainTitleBarLeadingInset(
            leftNavigationWidth: 44,
            titleBarNavigationOverlayWidth: 0,
            isFullScreen: true
        ) == 0)
    }
}

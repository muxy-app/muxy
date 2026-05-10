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
    @Test("visible sidebar owns the title bar height instead of sitting below tab strip")
    func visibleSidebarExtendsThroughTitleBar() {
        #expect(MainWindowLayout.leftNavigationWidth(
            sidebarWidth: 44,
            titleBarNavigationWidth: 127,
            isFullScreen: false
        ) == 127)
        #expect(!MainWindowLayout.needsMainTitleBarNavigationInset(
            leftNavigationWidth: 127,
            isFullScreen: false
        ))
    }

    @Test("hidden sidebar keeps title bar navigation inset in main title bar")
    func hiddenSidebarLeavesNavigationInTitleBar() {
        #expect(MainWindowLayout.leftNavigationWidth(
            sidebarWidth: 0,
            titleBarNavigationWidth: 127,
            isFullScreen: false
        ) == 0)
        #expect(MainWindowLayout.needsMainTitleBarNavigationInset(
            leftNavigationWidth: 0,
            isFullScreen: false
        ))
    }

    @Test("full screen uses sidebar width without traffic light padding")
    func fullScreenSidebarUsesResolvedWidth() {
        #expect(MainWindowLayout.leftNavigationWidth(
            sidebarWidth: 44,
            titleBarNavigationWidth: 127,
            isFullScreen: true
        ) == 44)
        #expect(!MainWindowLayout.needsMainTitleBarNavigationInset(
            leftNavigationWidth: 44,
            isFullScreen: true
        ))
    }
}

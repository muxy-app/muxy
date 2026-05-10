import Testing

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

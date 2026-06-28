import Foundation
import Testing

@testable import Muxy

@Suite("ProjectSortMode")
struct ProjectSortModeTests {
    private func project(_ name: String, sortOrder: Int = 0, createdAt: Date = Date(), lastActiveAt: Date? = nil, isPinned: Bool = false) -> Project {
        var project = Project(name: name, path: "/tmp/\(name)", sortOrder: sortOrder)
        project.createdAt = createdAt
        project.lastActiveAt = lastActiveAt
        project.isPinned = isPinned
        return project
    }

    @Test("manual preserves the given order")
    func manualPreservesOrder() {
        let input = [project("B", sortOrder: 0), project("A", sortOrder: 1)]
        #expect(ProjectSortMode.manual.sorted(input).map(\.name) == ["B", "A"])
    }

    @Test("name ascending uses natural ordering")
    func nameAscendingNaturalOrder() {
        let input = [project("repo10"), project("repo2"), project("Repo1")]
        #expect(ProjectSortMode.nameAscending.sorted(input).map(\.name) == ["Repo1", "repo2", "repo10"])
    }

    @Test("name descending reverses natural ordering")
    func nameDescendingNaturalOrder() {
        let input = [project("Repo1"), project("repo10"), project("repo2")]
        #expect(ProjectSortMode.nameDescending.sorted(input).map(\.name) == ["repo10", "repo2", "Repo1"])
    }

    @Test("recently active orders newest first with nils last")
    func recentlyActiveOrdersByTimestamp() {
        let old = Date(timeIntervalSince1970: 1000)
        let recent = Date(timeIntervalSince1970: 2000)
        let input = [
            project("Never", sortOrder: 0, lastActiveAt: nil),
            project("Old", sortOrder: 1, lastActiveAt: old),
            project("Recent", sortOrder: 2, lastActiveAt: recent),
        ]
        #expect(ProjectSortMode.recentlyActive.sorted(input).map(\.name) == ["Recent", "Old", "Never"])
    }

    @Test("recently active breaks ties by sortOrder when both nil")
    func recentlyActiveBreaksNilTies() {
        let input = [project("Second", sortOrder: 1), project("First", sortOrder: 0)]
        #expect(ProjectSortMode.recentlyActive.sorted(input).map(\.name) == ["First", "Second"])
    }

    @Test("date created orders oldest first")
    func dateCreatedOrdersByCreation() {
        let older = Date(timeIntervalSince1970: 1000)
        let newer = Date(timeIntervalSince1970: 2000)
        let input = [project("Newer", createdAt: newer), project("Older", createdAt: older)]
        #expect(ProjectSortMode.dateCreated.sorted(input).map(\.name) == ["Older", "Newer"])
    }

    @Test("pinned projects are placed ahead of unpinned in every mode")
    func pinnedFirst() {
        let input = [
            project("Apple", sortOrder: 0),
            project("Zebra", sortOrder: 1, isPinned: true),
            project("Mango", sortOrder: 2),
        ]
        #expect(ProjectSortMode.manual.sorted(input).map(\.name) == ["Zebra", "Apple", "Mango"])
        #expect(ProjectSortMode.nameAscending.sorted(input).map(\.name) == ["Zebra", "Apple", "Mango"])
    }

    @Test("pinned projects keep the chosen mode order among themselves")
    func pinnedPreserveModeOrder() {
        let input = [
            project("Bravo", isPinned: true),
            project("Alpha", isPinned: true),
            project("Charlie"),
        ]
        #expect(ProjectSortMode.nameAscending.sorted(input).map(\.name) == ["Alpha", "Bravo", "Charlie"])
    }
}

import Foundation

enum LayoutSidebar: Identifiable {
    case projectList
    case tabList

    var id: Self { self }
}

enum LayoutTopbar {
    case tabStrip
    case repositoryStatus
}

protocol AppLayoutProviding {
    var sidebars: [LayoutSidebar] { get }
    var topbar: LayoutTopbar { get }
}

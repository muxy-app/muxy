import Foundation

enum LayoutSidebar: Identifiable {
    case projectList
    case tabList
    case agentList

    var id: Self { self }
}

enum LayoutTopbar {
    case tabStrip
    case projectTitle
}

protocol AppLayoutProviding {
    var sidebars: [LayoutSidebar] { get }
    var topbar: LayoutTopbar { get }
}

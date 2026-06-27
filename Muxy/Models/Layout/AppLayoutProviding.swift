import Foundation

enum LayoutSidebar: Identifiable {
    case projectList
    case tabList

    var id: Self { self }
}

enum LayoutTopbar {
    case tabStrip
    case breadcrumb
}

protocol AppLayoutProviding {
    var sidebars: [LayoutSidebar] { get }
    var topbar: LayoutTopbar { get }
}

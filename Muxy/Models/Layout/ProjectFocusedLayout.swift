import Foundation

struct ProjectFocusedLayout: AppLayoutProviding {
    var sidebars: [LayoutSidebar] { [.projectList] }
    var topbar: LayoutTopbar { .tabStrip }
}

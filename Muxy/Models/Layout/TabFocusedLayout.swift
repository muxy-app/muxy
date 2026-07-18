import Foundation

struct TabFocusedLayout: AppLayoutProviding {
    var sidebars: [LayoutSidebar] { [.tabList] }
    var topbar: LayoutTopbar { .projectTitle }
}

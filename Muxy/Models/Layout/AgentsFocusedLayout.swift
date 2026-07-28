import Foundation

struct AgentsFocusedLayout: AppLayoutProviding {
    var sidebars: [LayoutSidebar] { [.agentList] }
    var topbar: LayoutTopbar { .tabStrip }
}

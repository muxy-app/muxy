import Foundation

enum ProjectCleanupPolicy {
    static func shouldRemoveStoredProjectWhenWorkspaceEmptied(_ project: Project) -> Bool {
        project.remoteDeviceID == nil
    }
}

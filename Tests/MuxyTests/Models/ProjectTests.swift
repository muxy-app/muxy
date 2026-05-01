import Foundation
import Testing

@testable import Muxy

@Suite("Project")
struct ProjectTests {
    @Test("Project decodes legacy records without worktree location")
    func projectLegacyDecodeDefaultsWorktreeLocation() throws {
        let json = """
        {
          "id": "\(UUID().uuidString)",
          "name": "Repo",
          "path": "/tmp/repo",
          "sortOrder": 0,
          "createdAt": "2024-01-01T00:00:00Z",
          "icon": null,
          "logo": null,
          "iconColor": null
        }
        """
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let project = try decoder.decode(Project.self, from: Data(json.utf8))

        #expect(project.preferredWorktreeParentPath == nil)
    }
}

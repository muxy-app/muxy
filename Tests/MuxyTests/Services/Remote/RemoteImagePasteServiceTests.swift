import Foundation
import Testing

@testable import Muxy

@Suite("Remote image paste")
struct RemoteImagePasteServiceTests {
    @Test("upload command creates a private PNG through standard input")
    func uploadCommand() {
        let command = RemoteImagePasteService.uploadCommand

        #expect(command.contains("umask 077"))
        #expect(command.contains("mktemp -d muxy-image.XXXXXXXX"))
        #expect(command.contains("cat > \"$__muxy_path\""))
        #expect(command.contains("chmod 600 \"$__muxy_path\""))
        #expect(command.contains("MUXY_IMAGE_PATH=%s"))
    }

    @Test("extracts a managed path after unrelated remote output")
    func extractsUploadedPath() throws {
        let path = "/tmp/muxy-image.a1B2c3D4/image.png"
        let result = GitProcessResult(
            status: 0,
            stdout: "shell banner\nMUXY_IMAGE_PATH=\(path)\n",
            stdoutData: Data(),
            stderr: "",
            truncated: false
        )

        #expect(try RemoteImagePasteService.uploadedPath(from: result) == path)
    }

    @Test("rejects paths outside a managed temporary directory")
    func rejectsUnmanagedPath() {
        let result = GitProcessResult(
            status: 0,
            stdout: "MUXY_IMAGE_PATH=/tmp/image.png\n",
            stdoutData: Data(),
            stderr: "",
            truncated: false
        )

        #expect(throws: RemoteImagePasteError.self) {
            try RemoteImagePasteService.uploadedPath(from: result)
        }
    }

    @Test("cleanup includes only exact managed files and directories")
    func cleanupCommand() throws {
        let first = "/tmp/muxy-image.a1B2c3D4/image.png"
        let second = "/var/tmp/muxy-image.e5F6g7H8/image.png"
        let command = try #require(RemoteImagePasteService.cleanupCommand(paths: [
            first,
            second,
            "/tmp/not-managed.png",
        ]))

        #expect(command.contains("rm -f \(first) \(second)"))
        #expect(command.contains("rmdir /tmp/muxy-image.a1B2c3D4 /var/tmp/muxy-image.e5F6g7H8"))
        #expect(!command.contains("not-managed"))
    }
}

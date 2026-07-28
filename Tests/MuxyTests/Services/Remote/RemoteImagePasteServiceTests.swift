import Foundation
import Testing

@testable import Muxy

@Suite("Remote image paste")
struct RemoteImagePasteServiceTests {
    private let sessionID = "01234567-89AB-CDEF-0123-456789ABCDEF"
    private let imageID = "ABCDEF01-2345-6789-ABCD-EF0123456789"

    @Test("upload command creates a private PNG through standard input")
    func uploadCommand() {
        let command = RemoteImagePasteService.uploadCommand(sessionID: sessionID, imageID: imageID)

        #expect(command.contains("umask 077"))
        #expect(command.contains("mkdir -m 700"))
        #expect(command.contains("muxy-images.\(sessionID)"))
        #expect(command.contains("\(imageID).png"))
        #expect(command.contains("cat > \"$__muxy_partial\""))
        #expect(command.contains("chmod 600 \"$__muxy_partial\""))
        #expect(command.contains("trap 'rm -f \"$__muxy_partial\"'"))
        #expect(command.contains("MUXY_IMAGE_PATH=%s"))
    }

    @Test("extracts a managed path after unrelated remote output")
    func extractsUploadedPath() throws {
        let path = "/tmp/muxy-images.\(sessionID)/\(imageID).png"
        let result = GitProcessResult(
            status: 0,
            stdout: "shell banner\nMUXY_IMAGE_PATH=\(path)\n",
            stdoutData: Data(),
            stderr: "",
            truncated: false
        )

        #expect(try RemoteImagePasteService.uploadedPath(
            from: result,
            sessionID: sessionID,
            imageID: imageID
        ) == path)
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
            try RemoteImagePasteService.uploadedPath(
                from: result,
                sessionID: sessionID,
                imageID: imageID
            )
        }
    }

    @Test("cleanup removes every image in the known session directory")
    func cleanupCommand() throws {
        let command = try #require(RemoteImagePasteService.cleanupCommand(sessionID: sessionID))

        #expect(command.contains("muxy-images.\(sessionID)"))
        #expect(command.contains("*.png"))
        #expect(command.contains("*.part"))
        #expect(command.contains("__muxy_status=$?"))
        #expect(command.hasSuffix("exit $__muxy_status"))
    }

    @Test("cleanup rejects unsafe session identifiers")
    func rejectsUnsafeCleanupIdentifier() {
        #expect(RemoteImagePasteService.cleanupCommand(sessionID: "../../tmp") == nil)
    }

    @Test("managed paths must match the exact session and image")
    func exactManagedPath() {
        let path = "/var/tmp/muxy-images.\(sessionID)/\(imageID).png"

        #expect(RemoteImagePasteService.isManagedPath(path, sessionID: sessionID, imageID: imageID))
        #expect(!RemoteImagePasteService.isManagedPath(
            path,
            sessionID: "FEDCBA98-7654-3210-FEDC-BA9876543210",
            imageID: imageID
        ))
        #expect(!RemoteImagePasteService.isManagedPath(
            path,
            sessionID: sessionID,
            imageID: "FEDCBA98-7654-3210-FEDC-BA9876543210"
        ))
    }
}

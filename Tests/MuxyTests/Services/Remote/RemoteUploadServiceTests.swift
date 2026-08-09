import Foundation
import Testing

@testable import Muxy

@Suite("Remote upload")
struct RemoteUploadServiceTests {
    private let sessionID = "01234567-89AB-CDEF-0123-456789ABCDEF"
    private let uploadID = "ABCDEF01-2345-6789-ABCD-EF0123456789"

    @Test("upload command creates a private file through standard input")
    func uploadCommand() {
        let command = RemoteUploadService.uploadCommand(
            sessionID: sessionID,
            uploadID: uploadID,
            fileExtension: "png"
        )

        #expect(command.contains("umask 077"))
        #expect(command.contains("mkdir -m 700"))
        #expect(command.contains("muxy-uploads.\(sessionID)"))
        #expect(command.contains("\(uploadID).png"))
        #expect(command.contains("cat > \"$__muxy_partial\""))
        #expect(command.contains("chmod 600 \"$__muxy_partial\""))
        #expect(command.contains("trap 'rm -f \"$__muxy_partial\"'"))
        #expect(command.contains("MUXY_UPLOAD_PATH=%s"))
    }

    @Test("upload command omits the separator when there is no extension")
    func uploadCommandWithoutExtension() {
        let command = RemoteUploadService.uploadCommand(
            sessionID: sessionID,
            uploadID: uploadID,
            fileExtension: nil
        )

        #expect(command.contains("/\(uploadID)\""))
        #expect(!command.contains("\(uploadID)."))
    }

    @Test("extracts a managed path after unrelated remote output")
    func extractsUploadedPath() throws {
        let path = "/tmp/muxy-uploads.\(sessionID)/\(uploadID).pdf"
        let result = GitProcessResult(
            status: 0,
            stdout: "shell banner\nMUXY_UPLOAD_PATH=\(path)\n",
            stdoutData: Data(),
            stderr: "",
            truncated: false
        )

        #expect(try RemoteUploadService.uploadedPath(
            from: result,
            sessionID: sessionID,
            uploadID: uploadID,
            fileExtension: "pdf"
        ) == path)
    }

    @Test("rejects paths outside a managed temporary directory")
    func rejectsUnmanagedPath() {
        let result = GitProcessResult(
            status: 0,
            stdout: "MUXY_UPLOAD_PATH=/tmp/report.pdf\n",
            stdoutData: Data(),
            stderr: "",
            truncated: false
        )

        #expect(throws: RemoteUploadError.self) {
            try RemoteUploadService.uploadedPath(
                from: result,
                sessionID: sessionID,
                uploadID: uploadID,
                fileExtension: "pdf"
            )
        }
    }

    @Test("rejects a path whose extension does not match the request")
    func rejectsMismatchedExtension() {
        let result = GitProcessResult(
            status: 0,
            stdout: "MUXY_UPLOAD_PATH=/tmp/muxy-uploads.\(sessionID)/\(uploadID).sh\n",
            stdoutData: Data(),
            stderr: "",
            truncated: false
        )

        #expect(throws: RemoteUploadError.self) {
            try RemoteUploadService.uploadedPath(
                from: result,
                sessionID: sessionID,
                uploadID: uploadID,
                fileExtension: "pdf"
            )
        }
    }

    @Test("cleanup removes every upload in the known session directory")
    func cleanupCommand() throws {
        let command = try #require(RemoteUploadService.cleanupCommand(sessionID: sessionID))

        #expect(command.contains("muxy-uploads.\(sessionID)"))
        #expect(command.contains("rm -f \"$__muxy_dir\"/*"))
        #expect(command.contains("__muxy_status=$?"))
        #expect(command.hasSuffix("exit $__muxy_status"))
    }

    @Test("cleanup rejects unsafe session identifiers")
    func rejectsUnsafeCleanupIdentifier() {
        #expect(RemoteUploadService.cleanupCommand(sessionID: "../../tmp") == nil)
    }

    @Test("managed paths must match the exact session and upload")
    func exactManagedPath() {
        let path = "/var/tmp/muxy-uploads.\(sessionID)/\(uploadID).png"

        #expect(RemoteUploadService.isManagedPath(
            path,
            sessionID: sessionID,
            uploadID: uploadID,
            fileExtension: "png"
        ))
        #expect(!RemoteUploadService.isManagedPath(
            path,
            sessionID: "FEDCBA98-7654-3210-FEDC-BA9876543210",
            uploadID: uploadID,
            fileExtension: "png"
        ))
        #expect(!RemoteUploadService.isManagedPath(
            path,
            sessionID: sessionID,
            uploadID: "FEDCBA98-7654-3210-FEDC-BA9876543210",
            fileExtension: "png"
        ))
    }

    @Test("extensions are lowercased and rejected when not alphanumeric")
    func sanitizesExtensions() {
        #expect(RemoteUploadService.sanitizedExtension(for: URL(fileURLWithPath: "/a/report.PDF")) == "pdf")
        #expect(RemoteUploadService.sanitizedExtension(for: URL(fileURLWithPath: "/a/archive.tar.gz")) == "gz")
        #expect(RemoteUploadService.sanitizedExtension(for: URL(fileURLWithPath: "/a/Makefile")) == nil)
        #expect(RemoteUploadService.sanitizedExtension(for: URL(fileURLWithPath: "/a/weird.c++")) == nil)
        #expect(RemoteUploadService.sanitizedExtension(
            for: URL(fileURLWithPath: "/a/weird.abcdefghijklmnopq")
        ) == nil)
    }

    @Test("timeout grows with payload size but never drops below the default")
    func scalesTimeout() {
        #expect(RemoteUploadService.timeout(forByteCount: 0) == SSHCommandRunner.defaultTimeout)
        #expect(RemoteUploadService.timeout(forByteCount: 1024) == SSHCommandRunner.defaultTimeout)
        #expect(
            RemoteUploadService.timeout(forByteCount: RemoteUploadService.maximumByteCount)
                > SSHCommandRunner.defaultTimeout
        )
    }
}

import Darwin

enum SessionSocket {
    static let backlog: Int32 = 16

    static func makeListener(path: String) -> Int32? {
        guard let address = makeAddress(path: path) else {
            SessionLog.write("socket path too long: \(path)")
            return nil
        }
        unlink(path)
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { return nil }
        SessionIO.setCloseOnExec(descriptor)

        var storage = address
        let previousMask = umask(0o077)
        let bound = withUnsafePointer(to: &storage) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { casted in
                bind(descriptor, casted, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        umask(previousMask)
        guard bound == 0 else {
            SessionLog.write("bind failed: \(String(cString: strerror(errno)))")
            SessionIO.close(descriptor)
            return nil
        }
        chmod(path, 0o600)
        guard listen(descriptor, backlog) == 0 else {
            SessionLog.write("listen failed: \(String(cString: strerror(errno)))")
            SessionIO.close(descriptor)
            return nil
        }
        SessionIO.setNonBlocking(descriptor)
        return descriptor
    }

    static func connect(path: String) -> Int32? {
        guard let address = makeAddress(path: path) else { return nil }
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { return nil }
        SessionIO.setCloseOnExec(descriptor)

        var storage = address
        let connected = withUnsafePointer(to: &storage) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { casted in
                Darwin.connect(descriptor, casted, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard connected == 0 else {
            SessionIO.close(descriptor)
            return nil
        }
        return descriptor
    }

    static func accept(_ listener: Int32) -> Int32? {
        let descriptor = Darwin.accept(listener, nil, nil)
        guard descriptor >= 0 else { return nil }
        SessionIO.setCloseOnExec(descriptor)
        SessionIO.setNonBlocking(descriptor)
        return descriptor
    }

    static func peerUserID(_ descriptor: Int32) -> uid_t? {
        var credentials = xucred()
        var length = socklen_t(MemoryLayout<xucred>.size)
        let result = withUnsafeMutablePointer(to: &credentials) { pointer in
            getsockopt(descriptor, SOL_LOCAL, LOCAL_PEERCRED, pointer, &length)
        }
        guard result == 0, credentials.cr_version == XUCRED_VERSION else { return nil }
        return credentials.cr_uid
    }

    static func acquireLock(path: String) -> Int32? {
        let descriptor = open(path, O_CREAT | O_RDWR, 0o600)
        guard descriptor >= 0 else { return nil }
        SessionIO.setCloseOnExec(descriptor)
        guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else {
            SessionIO.close(descriptor)
            return nil
        }
        return descriptor
    }

    private static func makeAddress(path: String) -> sockaddr_un? {
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let capacity = MemoryLayout.size(ofValue: address.sun_path)
        let bytes = Array(path.utf8)
        guard bytes.count < capacity else { return nil }
        withUnsafeMutablePointer(to: &address.sun_path) { pointer in
            pointer.withMemoryRebound(to: CChar.self, capacity: capacity) { destination in
                for (index, byte) in bytes.enumerated() {
                    destination[index] = CChar(bitPattern: byte)
                }
                destination[bytes.count] = 0
            }
        }
        address.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)
        return address
    }
}

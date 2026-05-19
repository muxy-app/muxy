import Darwin
import Foundation

enum PrivateNetworkGuard {
    static func isLiteralPrivateAddress(_ host: String) -> Bool {
        let stripped = stripBrackets(host)
        if let v4 = parseIPv4(stripped) { return isPrivateIPv4(v4) }
        if let v6 = parseIPv6(stripped) { return isPrivateIPv6(v6) }
        return false
    }

    static func isLiteralIPAddress(_ host: String) -> Bool {
        let stripped = stripBrackets(host)
        return parseIPv4(stripped) != nil || parseIPv6(stripped) != nil
    }

    static func hostResolvesToPublicAddress(_ host: String) -> Bool {
        guard !host.isEmpty else { return false }
        let stripped = stripBrackets(host)
        if let v4 = parseIPv4(stripped) { return !isPrivateIPv4(v4) }
        if let v6 = parseIPv6(stripped) { return !isPrivateIPv6(v6) }

        var hints = addrinfo()
        hints.ai_family = AF_UNSPEC
        hints.ai_socktype = SOCK_STREAM

        var result: UnsafeMutablePointer<addrinfo>?
        let status = getaddrinfo(stripped, nil, &hints, &result)
        guard status == 0, let head = result else { return false }
        defer { freeaddrinfo(head) }

        var sawAny = false
        var ptr: UnsafeMutablePointer<addrinfo>? = head
        while let info = ptr {
            sawAny = true
            if let addr = info.pointee.ai_addr {
                if !isPublicSockAddr(addr) { return false }
            }
            ptr = info.pointee.ai_next
        }
        return sawAny
    }

    private static func stripBrackets(_ host: String) -> String {
        if host.hasPrefix("["), host.hasSuffix("]") {
            return String(host.dropFirst().dropLast())
        }
        return host
    }

    private static func parseIPv4(_ s: String) -> in_addr? {
        var addr = in_addr()
        let ok = s.withCString { inet_pton(AF_INET, $0, &addr) }
        return ok == 1 ? addr : nil
    }

    private static func parseIPv6(_ s: String) -> in6_addr? {
        var addr = in6_addr()
        let ok = s.withCString { inet_pton(AF_INET6, $0, &addr) }
        return ok == 1 ? addr : nil
    }

    private static func isPrivateIPv4(_ addr: in_addr) -> Bool {
        let bytes = withUnsafeBytes(of: addr.s_addr) { Array($0) }
        guard bytes.count == 4 else { return true }
        let b0 = bytes[0]
        let b1 = bytes[1]
        if b0 == 0 { return true }
        if b0 == 10 { return true }
        if b0 == 127 { return true }
        if b0 == 169, b1 == 254 { return true }
        if b0 == 172, (16 ... 31).contains(b1) { return true }
        if b0 == 192, b1 == 168 { return true }
        if b0 == 100, (64 ... 127).contains(b1) { return true }
        return false
    }

    private static func isPrivateIPv6(_ addr: in6_addr) -> Bool {
        let bytes = withUnsafeBytes(of: addr) { Array($0) }
        guard bytes.count == 16 else { return true }
        if bytes == Array(repeating: 0, count: 16) { return true }
        if bytes == Array(repeating: 0, count: 15) + [1] { return true }
        if bytes[0] == 0xFE, (bytes[1] & 0xC0) == 0x80 { return true }
        if (bytes[0] & 0xFE) == 0xFC { return true }
        if bytes[0 ... 9] == ArraySlice(Array(repeating: UInt8(0), count: 10)),
           bytes[10] == 0xFF, bytes[11] == 0xFF
        {
            var mapped = in_addr()
            mapped.s_addr = (UInt32(bytes[12]) << 24)
                | (UInt32(bytes[13]) << 16)
                | (UInt32(bytes[14]) << 8)
                | UInt32(bytes[15])
            mapped.s_addr = mapped.s_addr.bigEndian
            return isPrivateIPv4(mapped)
        }
        return false
    }

    private static func isPublicSockAddr(_ addr: UnsafePointer<sockaddr>) -> Bool {
        switch Int32(addr.pointee.sa_family) {
        case AF_INET:
            let v4 = addr.withMemoryRebound(to: sockaddr_in.self, capacity: 1) { $0.pointee.sin_addr }
            return !isPrivateIPv4(v4)
        case AF_INET6:
            let v6 = addr.withMemoryRebound(to: sockaddr_in6.self, capacity: 1) { $0.pointee.sin6_addr }
            return !isPrivateIPv6(v6)
        default:
            return false
        }
    }
}

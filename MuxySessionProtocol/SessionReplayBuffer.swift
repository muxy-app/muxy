public struct SessionReplayBuffer: Sendable {
    public let capacity: Int

    private var storage: [UInt8]
    private var start = 0
    private var count = 0

    public init(capacity: Int) {
        self.capacity = max(capacity, 0)
        storage = [UInt8](repeating: 0, count: self.capacity)
    }

    public var isEmpty: Bool { count == 0 }
    public var byteCount: Int { count }

    public mutating func append(_ bytes: [UInt8]) {
        bytes.withUnsafeBufferPointer { append($0) }
    }

    public mutating func append(_ bytes: UnsafeBufferPointer<UInt8>) {
        guard capacity > 0, !bytes.isEmpty else { return }
        guard bytes.count < capacity else {
            let tail = bytes.suffix(capacity)
            for (offset, byte) in tail.enumerated() {
                storage[offset] = byte
            }
            start = 0
            count = capacity
            return
        }
        for byte in bytes {
            storage[(start + count) % capacity] = byte
            if count < capacity {
                count += 1
            } else {
                start = (start + 1) % capacity
            }
        }
    }

    public var bytes: [UInt8] {
        guard count > 0 else { return [] }
        var result = [UInt8]()
        result.reserveCapacity(count)
        for offset in 0 ..< count {
            result.append(storage[(start + offset) % capacity])
        }
        return result
    }

    public mutating func removeAll() {
        start = 0
        count = 0
    }
}

public struct SessionReplayBuffer: Sendable {
    private static let alternateScreenEnterSequences: [[UInt8]] = [
        [0x1B, 0x5B, 0x3F, 0x34, 0x37, 0x68],
        [0x1B, 0x5B, 0x3F, 0x31, 0x30, 0x34, 0x37, 0x68],
        [0x1B, 0x5B, 0x3F, 0x31, 0x30, 0x34, 0x39, 0x68],
    ]

    private static let alternateScreenLeaveSequences: [[UInt8]] = [
        [0x1B, 0x5B, 0x3F, 0x34, 0x37, 0x6C],
        [0x1B, 0x5B, 0x3F, 0x31, 0x30, 0x34, 0x37, 0x6C],
        [0x1B, 0x5B, 0x3F, 0x31, 0x30, 0x34, 0x39, 0x6C],
    ]

    private static let screenControlTailLength = max(
        alternateScreenEnterSequences.map(\.count).max() ?? 0,
        alternateScreenLeaveSequences.map(\.count).max() ?? 0
    )

    public let capacity: Int

    private var storage: [UInt8]
    private var start = 0
    private var count = 0
    private var hasDiscardedBytes = false
    private var alternateScreenActive = false
    private var screenControlTail: [UInt8] = []

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
        appendForReplay(Array(bytes))
    }

    public var isAlternateScreenActive: Bool { alternateScreenActive }

    public var replayBytes: [UInt8] {
        guard !alternateScreenActive else { return [] }
        var output = bytes
        var needsLeadingFragmentCleanup = false
        if hasDiscardedBytes {
            let start = safeReplayStart(in: output)
            needsLeadingFragmentCleanup = start == output.startIndex
            output = Array(output[start...])
        }
        if needsLeadingFragmentCleanup {
            let leading = leadingSafeIndex(in: output)
            guard leading < output.count else { return [] }
            output = Array(output[leading...])
        }
        let trailing = trailingSafeEnd(in: output)
        guard trailing > 0 else { return [] }
        return Array(output[..<trailing])
    }

    private mutating func appendForReplay(_ bytes: [UInt8]) {
        let previousTail = screenControlTail
        let combined = previousTail + bytes
        let newByteOffset = previousTail.count
        var index = 0
        var unhandledNewByteIndex = 0
        while index < combined.count {
            if alternateScreenActive {
                guard let leave = nextAlternateScreenSequence(in: combined, from: index, entering: false) else {
                    updateScreenControlTail(from: combined)
                    return
                }
                removeAll()
                alternateScreenActive = false
                unhandledNewByteIndex = max(unhandledNewByteIndex, max(0, leave.upperBound - newByteOffset))
                index = leave.upperBound
                continue
            }

            guard let next = nextAlternateScreenSequence(in: combined, from: index, entering: true) else {
                if unhandledNewByteIndex < bytes.count {
                    appendStorage(Array(bytes[unhandledNewByteIndex...]))
                }
                updateScreenControlTail(from: combined)
                return
            }
            removeAll()
            alternateScreenActive = true
            unhandledNewByteIndex = max(unhandledNewByteIndex, max(0, next.upperBound - newByteOffset))
            index = next.upperBound
        }
        if !alternateScreenActive, unhandledNewByteIndex < bytes.count {
            appendStorage(Array(bytes[unhandledNewByteIndex...]))
        }
        updateScreenControlTail(from: combined)
    }

    private mutating func appendStorage(_ bytes: [UInt8]) {
        guard capacity > 0, !bytes.isEmpty else { return }
        guard bytes.count <= capacity else {
            let tail = bytes.suffix(capacity)
            for (offset, byte) in tail.enumerated() {
                storage[offset] = byte
            }
            start = 0
            count = capacity
            hasDiscardedBytes = true
            return
        }
        if bytes.count == capacity {
            let discardedExistingBytes = count > 0
            for (offset, byte) in bytes.enumerated() {
                storage[offset] = byte
            }
            start = 0
            count = capacity
            if discardedExistingBytes {
                hasDiscardedBytes = true
            }
            return
        }
        for byte in bytes {
            storage[(start + count) % capacity] = byte
            if count < capacity {
                count += 1
            } else {
                start = (start + 1) % capacity
                hasDiscardedBytes = true
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
        hasDiscardedBytes = false
        alternateScreenActive = false
        screenControlTail = []
    }

    private mutating func updateScreenControlTail(from bytes: [UInt8]) {
        screenControlTail = Array(bytes.suffix(Self.screenControlTailLength))
    }

    private func safeReplayStart(in bytes: [UInt8]) -> Int {
        guard !bytes.isEmpty else { return 0 }
        guard let newline = bytes.firstIndex(where: { $0 == 0x0A || $0 == 0x0D }) else { return 0 }
        return bytes.index(after: newline)
    }

    private func leadingSafeIndex(in bytes: [UInt8]) -> Int {
        var index = 0
        while index < bytes.count {
            let byte = bytes[index]
            if byte >= 0x80, byte <= 0xBF {
                index += 1
                continue
            }
            if byte == 0x5D {
                guard isLikelyBareOSCBody(in: bytes, from: index) else { return index }
                guard let end = oscTerminator(in: bytes, from: index + 1) else { return bytes.count }
                index = end
                continue
            }
            if byte == 0x5B {
                guard isLikelyBareCSIFragment(in: bytes, from: index) else { return index }
                guard let end = csiTerminator(in: bytes, from: index + 1) else { return bytes.count }
                index = end
                continue
            }
            return index
        }
        return index
    }

    private func isLikelyBareOSCBody(in bytes: [UInt8], from index: Int) -> Bool {
        var cursor = index + 1
        var sawDigit = false
        while cursor < bytes.count, bytes[cursor] >= 0x30, bytes[cursor] <= 0x39 {
            sawDigit = true
            cursor += 1
        }
        return sawDigit && cursor < bytes.count && bytes[cursor] == 0x3B
    }

    private func isLikelyBareCSIFragment(in bytes: [UInt8], from index: Int) -> Bool {
        let next = index + 1
        guard next < bytes.count else { return false }
        return (bytes[next] >= 0x30 && bytes[next] <= 0x3F) || (bytes[next] >= 0x20 && bytes[next] <= 0x2F)
    }

    private func trailingSafeEnd(in bytes: [UInt8]) -> Int {
        var index = 0
        while index < bytes.count {
            guard bytes[index] == 0x1B else {
                index += 1
                continue
            }
            guard let end = escapeTerminator(in: bytes, from: index) else {
                return trailingUTF8SafeEnd(in: bytes, endingAt: index)
            }
            index = end
        }
        return trailingUTF8SafeEnd(in: bytes, endingAt: bytes.count)
    }

    private func trailingUTF8SafeEnd(in bytes: [UInt8], endingAt end: Int) -> Int {
        guard end > 0 else { return 0 }
        var sequenceStart = end - 1
        var continuationCount = 0
        while sequenceStart > 0,
              isUTF8Continuation(bytes[sequenceStart]),
              continuationCount < 3
        {
            sequenceStart -= 1
            continuationCount += 1
        }
        guard let expectedLength = utf8SequenceLength(startingWith: bytes[sequenceStart]) else { return end }
        let actualLength = end - sequenceStart
        guard actualLength < expectedLength else { return end }
        guard bytes[(sequenceStart + 1) ..< end].allSatisfy(isUTF8Continuation) else { return end }
        return sequenceStart
    }

    private func utf8SequenceLength(startingWith byte: UInt8) -> Int? {
        switch byte {
        case 0x00 ... 0x7F:
            1
        case 0xC2 ... 0xDF:
            2
        case 0xE0 ... 0xEF:
            3
        case 0xF0 ... 0xF4:
            4
        default:
            nil
        }
    }

    private func isUTF8Continuation(_ byte: UInt8) -> Bool {
        byte >= 0x80 && byte <= 0xBF
    }

    private func escapeTerminator(in bytes: [UInt8], from index: Int) -> Int? {
        let next = index + 1
        guard next < bytes.count else { return nil }
        switch bytes[next] {
        case 0x5D:
            return oscTerminator(in: bytes, from: next + 1)
        case 0x50,
             0x58,
             0x5E,
             0x5F:
            return stringControlTerminator(in: bytes, from: next + 1)
        case 0x5B:
            return csiTerminator(in: bytes, from: next + 1)
        case 0x20 ... 0x2F:
            var cursor = next + 1
            while cursor < bytes.count {
                if bytes[cursor] >= 0x30, bytes[cursor] <= 0x7E {
                    return cursor + 1
                }
                cursor += 1
            }
            return nil
        default:
            return next + 1
        }
    }

    private func oscTerminator(in bytes: [UInt8], from index: Int) -> Int? {
        var cursor = index
        while cursor < bytes.count {
            if bytes[cursor] == 0x07 {
                return cursor + 1
            }
            if bytes[cursor] == 0x1B, cursor + 1 < bytes.count, bytes[cursor + 1] == 0x5C {
                return cursor + 2
            }
            cursor += 1
        }
        return nil
    }

    private func stringControlTerminator(in bytes: [UInt8], from index: Int) -> Int? {
        var cursor = index
        while cursor + 1 < bytes.count {
            if bytes[cursor] == 0x1B, bytes[cursor + 1] == 0x5C {
                return cursor + 2
            }
            cursor += 1
        }
        return nil
    }

    private func csiTerminator(in bytes: [UInt8], from index: Int) -> Int? {
        var cursor = index
        while cursor < bytes.count {
            if bytes[cursor] >= 0x40, bytes[cursor] <= 0x7E {
                return cursor + 1
            }
            cursor += 1
        }
        return nil
    }

    private func nextAlternateScreenSequence(in bytes: [UInt8], from index: Int, entering: Bool) -> Range<Int>? {
        let sequences = entering ? Self.alternateScreenEnterSequences : Self.alternateScreenLeaveSequences
        var best: Range<Int>?
        for sequence in sequences {
            guard let range = firstRange(of: sequence, in: bytes, from: index) else { continue }
            if best == nil || range.lowerBound < best!.lowerBound {
                best = range
            }
        }
        return best
    }

    private func firstRange(of needle: [UInt8], in bytes: [UInt8], from index: Int) -> Range<Int>? {
        guard !needle.isEmpty, index >= 0, bytes.count - index >= needle.count else { return nil }
        var cursor = index
        while cursor + needle.count <= bytes.count {
            var offset = 0
            var matches = true
            while offset < needle.count {
                if bytes[cursor + offset] != needle[offset] {
                    matches = false
                    break
                }
                offset += 1
            }
            if matches {
                return cursor ..< cursor + needle.count
            }
            cursor += 1
        }
        return nil
    }
}

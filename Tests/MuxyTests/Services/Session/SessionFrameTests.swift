import MuxySessionProtocol
import Testing

@Suite("SessionFrame")
struct SessionFrameTests {
    @Test("encodes a header of kind and big-endian length")
    func encodesHeader() {
        let frame = SessionFrame(kind: .input, payload: [0xAA, 0xBB, 0xCC])
        #expect(frame.encoded() == [0x02, 0x00, 0x00, 0x00, 0x03, 0xAA, 0xBB, 0xCC])
    }

    @Test("decodes frames pushed in a single chunk")
    func decodesSingleChunk() throws {
        var decoder = SessionFrameDecoder()
        decoder.push(SessionFrame(kind: .list).encoded())
        decoder.push(SessionFrame(kind: .output, payload: [1, 2, 3]).encoded())

        #expect(try decoder.next() == SessionFrame(kind: .list))
        #expect(try decoder.next() == SessionFrame(kind: .output, payload: [1, 2, 3]))
        #expect(try decoder.next() == nil)
    }

    @Test("decodes a frame split across arbitrary byte boundaries")
    func decodesSplitFrames() throws {
        let encoded = SessionFrame(kind: .output, payload: Array(repeating: 0x7F, count: 300)).encoded()
        var decoder = SessionFrameDecoder()
        for byte in encoded.dropLast() {
            decoder.push([byte])
            #expect(try decoder.next() == nil)
        }
        decoder.push([encoded[encoded.count - 1]])
        let frame = try decoder.next()
        #expect(frame?.kind == .output)
        #expect(frame?.payload.count == 300)
    }

    @Test("reports an empty payload frame as complete once the header arrives")
    func decodesEmptyPayload() throws {
        var decoder = SessionFrameDecoder()
        decoder.push([SessionFrameKind.acknowledged.rawValue, 0, 0, 0, 0])
        #expect(try decoder.next() == SessionFrame(kind: .acknowledged))
    }

    @Test("rejects an unknown frame kind")
    func rejectsUnknownKind() {
        var decoder = SessionFrameDecoder()
        decoder.push([0xF0, 0, 0, 0, 0])
        #expect(throws: SessionProtocolError.unknownFrameKind(0xF0)) {
            try decoder.next()
        }
    }

    @Test("rejects a frame larger than the maximum payload size")
    func rejectsOversizedFrame() {
        var decoder = SessionFrameDecoder()
        let length = SessionFrame.maximumPayloadSize + 1
        decoder.push([
            SessionFrameKind.input.rawValue,
            UInt8(truncatingIfNeeded: length >> 24),
            UInt8(truncatingIfNeeded: length >> 16),
            UInt8(truncatingIfNeeded: length >> 8),
            UInt8(truncatingIfNeeded: length),
        ])
        #expect(throws: SessionProtocolError.frameTooLarge(length)) {
            try decoder.next()
        }
    }

    @Test("checks the length before the kind so a hostile peer cannot force a large buffer")
    func checksLengthBeforeKind() {
        var decoder = SessionFrameDecoder()
        let length = SessionFrame.maximumPayloadSize + 1
        decoder.push([
            0xF0,
            UInt8(truncatingIfNeeded: length >> 24),
            UInt8(truncatingIfNeeded: length >> 16),
            UInt8(truncatingIfNeeded: length >> 8),
            UInt8(truncatingIfNeeded: length),
        ])
        #expect(throws: SessionProtocolError.frameTooLarge(length)) {
            try decoder.next()
        }
    }

    @Test("drains buffered bytes as frames are consumed")
    func drainsBuffer() throws {
        var decoder = SessionFrameDecoder()
        decoder.push(SessionFrame(kind: .input, payload: [9]).encoded())
        #expect(decoder.bufferedByteCount == 6)
        _ = try decoder.next()
        #expect(decoder.bufferedByteCount == 0)
    }
}

@Suite("SessionWindowSizePolicy")
struct SessionWindowSizePolicyTests {
    @Test("classifies usable terminal sizes")
    func classifiesUsableSizes() {
        #expect(!SessionWindowSizePolicy.isUsable(columns: 0, rows: 0))
        #expect(!SessionWindowSizePolicy.isUsable(columns: 1, rows: 1))
        #expect(!SessionWindowSizePolicy.isUsable(columns: 9, rows: 4))
        #expect(!SessionWindowSizePolicy.isUsable(columns: 10, rows: 3))
        #expect(SessionWindowSizePolicy.isUsable(columns: 10, rows: 4))
        #expect(SessionWindowSizePolicy.isUsable(columns: 80, rows: 24))
    }

    @Test("maps transient attach sizes to unknown")
    func mapsTransientAttachSizesToUnknown() {
        #expect(SessionWindowSizePolicy.attachSize(from: nil).columns == 0)
        #expect(SessionWindowSizePolicy.attachSize(from: (columns: 1, rows: 1)).rows == 0)
        #expect(SessionWindowSizePolicy.attachSize(from: (columns: 80, rows: 24)).columns == 80)
    }

    @Test("maps invalid create sizes to the fallback pty size")
    func mapsInvalidCreateSizesToFallback() {
        #expect(SessionWindowSizePolicy.createSize(columns: 0, rows: 0).columns == 80)
        #expect(SessionWindowSizePolicy.createSize(columns: 1, rows: 1).rows == 24)
        #expect(SessionWindowSizePolicy.createSize(columns: 120, rows: 40).columns == 120)
    }
}

@Suite("SessionIdentifier")
struct SessionIdentifierTests {
    @Test("round-trips a canonical uuid string")
    func roundTripsUUIDString() throws {
        let text = "3F2504E0-4F89-11D3-9A0C-0305E82C3301"
        let identifier = try #require(SessionIdentifier(uuidString: text))
        #expect(identifier.bytes.count == 16)
        #expect(identifier.uuidString == text.lowercased())
    }

    @Test("parses lowercase and uppercase equivalently")
    func parsesEitherCase() {
        let upper = SessionIdentifier(uuidString: "3F2504E0-4F89-11D3-9A0C-0305E82C3301")
        let lower = SessionIdentifier(uuidString: "3f2504e0-4f89-11d3-9a0c-0305e82c3301")
        #expect(upper == lower)
    }

    @Test("rejects malformed input")
    func rejectsMalformedInput() {
        #expect(SessionIdentifier(uuidString: "") == nil)
        #expect(SessionIdentifier(uuidString: "not-a-uuid") == nil)
        #expect(SessionIdentifier(uuidString: "3f2504e0-4f89-11d3-9a0c-0305e82c33") == nil)
        #expect(SessionIdentifier(uuidString: "3f2504e0-4f89-11d3-9a0c-0305e82c3301ff") == nil)
        #expect(SessionIdentifier(uuidString: "3f2504e0-4f89-11d3-9a0c-0305e82c330g") == nil)
        #expect(SessionIdentifier(bytes: [1, 2, 3]) == nil)
    }
}

@Suite("SessionMessages")
struct SessionMessagesTests {
    private func makeIdentifier() throws -> SessionIdentifier {
        try #require(SessionIdentifier(uuidString: "3f2504e0-4f89-11d3-9a0c-0305e82c3301"))
    }

    @Test("round-trips an attach request including its environment")
    func roundTripsAttachRequest() throws {
        let identifier = try makeIdentifier()
        let request = SessionAttachRequest(
            identifier: identifier,
            columns: 120,
            rows: 40,
            workingDirectory: "/Users/test/project",
            command: "/bin/zsh -l -c 'echo hi' /bin/zsh",
            shell: "/bin/zsh",
            resourcesDirectory: "/Applications/Muxy.app/Contents/Resources/ghostty",
            environment: [
                SessionEnvironmentEntry(key: "TERM", value: "xterm-ghostty"),
                SessionEnvironmentEntry(key: "MUXY_PANE_ID", value: identifier.uuidString),
            ]
        )
        #expect(try SessionAttachRequest.decode(request.encoded()) == request)
    }

    @Test("round-trips an attach request with no environment")
    func roundTripsEmptyEnvironment() throws {
        let identifier = try makeIdentifier()
        let request = SessionAttachRequest(
            identifier: identifier,
            columns: 80,
            rows: 24,
            workingDirectory: "/",
            command: "",
            shell: "/bin/zsh",
            resourcesDirectory: "",
            environment: []
        )
        #expect(try SessionAttachRequest.decode(request.encoded()) == request)
    }

    @Test("preserves non-ascii payloads")
    func preservesNonASCII() throws {
        let identifier = try makeIdentifier()
        let request = SessionAttachRequest(
            identifier: identifier,
            columns: 80,
            rows: 24,
            workingDirectory: "/Users/test/проект/日本語",
            command: "",
            shell: "/bin/zsh",
            resourcesDirectory: "",
            environment: [SessionEnvironmentEntry(key: "GREETING", value: "héllo 👋")]
        )
        #expect(try SessionAttachRequest.decode(request.encoded()) == request)
    }

    @Test("round-trips an attach acceptance")
    func roundTripsAttachAccepted() throws {
        let accepted = SessionAttachAccepted(created: true, shellProcessID: 4321, ttyDevice: 268_435_460)
        #expect(try SessionAttachAccepted.decode(accepted.encoded()) == accepted)
    }

    @Test("round-trips a descriptor list")
    func roundTripsDescriptorList() throws {
        let identifier = try makeIdentifier()
        let descriptors = [
            SessionDescriptor(
                identifier: identifier,
                shellProcessID: 900,
                ttyDevice: 42,
                workingDirectory: "/tmp",
                isAttached: true
            ),
            SessionDescriptor(
                identifier: identifier,
                shellProcessID: 901,
                ttyDevice: 43,
                workingDirectory: "/var",
                isAttached: false
            ),
        ]
        #expect(try SessionDescriptor.decodeList(SessionDescriptor.encodeList(descriptors)) == descriptors)
    }

    @Test("round-trips an empty descriptor list")
    func roundTripsEmptyDescriptorList() throws {
        #expect(try SessionDescriptor.decodeList(SessionDescriptor.encodeList([])).isEmpty)
    }

    @Test("round-trips resize, exit, text and identifier payloads")
    func roundTripsScalarPayloads() throws {
        let identifier = try makeIdentifier()
        let resize = try SessionResizePayload.decode(SessionResizePayload.encode(columns: 200, rows: 60))
        #expect(resize.columns == 200)
        #expect(resize.rows == 60)
        #expect(try SessionExitPayload.decode(SessionExitPayload.encode(status: -1)) == -1)
        #expect(try SessionTextPayload.decode(SessionTextPayload.encode("failed")) == "failed")
        #expect(try SessionIdentifierPayload.decode(SessionIdentifierPayload.encode(identifier)) == identifier)
    }

    @Test("rejects truncated payloads instead of trapping")
    func rejectsTruncatedPayloads() {
        #expect(throws: SessionProtocolError.malformedPayload) {
            try SessionAttachAccepted.decode([1, 2])
        }
        #expect(throws: SessionProtocolError.malformedPayload) {
            try SessionIdentifierPayload.decode([1, 2, 3])
        }
        #expect(throws: SessionProtocolError.malformedPayload) {
            try SessionTextPayload.decode([0, 0, 0, 8, 1, 2])
        }
    }
}

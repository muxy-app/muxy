import CryptoKit
import Darwin
import Foundation
import NIOCore
import NIOPosix
import NIOSSH
import os
import Security

private let sshLogger = Logger(subsystem: "app.muxy", category: "SSHConnection")

struct NativeSSHConnectionCallbacks {
    let onError: @MainActor (SSHConnectionError) -> Void
    let onClose: @MainActor () -> Void
}

@MainActor
final class SSHConnectionService {
    static let shared = SSHConnectionService()

    private let group = MultiThreadedEventLoopGroup(numberOfThreads: 1)
    private var connections: [UUID: NativeSSHConnection] = [:]

    private init() {}

    func start(
        paneID: UUID,
        configuration: NativeSSHConnectionConfiguration,
        bridge: NativeSSHFileDescriptorBridge,
        size: NativeSSHTerminalSize,
        callbacks: NativeSSHConnectionCallbacks
    ) {
        sshLogger
            .info(
                "Starting native SSH pane=\(paneID.uuidString, privacy: .public) size=\(size.columns)x\(size.rows)"
            )
        stop(paneID: paneID)
        let connection = NativeSSHConnection(
            paneID: paneID,
            configuration: configuration,
            bridge: bridge,
            size: size,
            group: group,
            callbacks: callbacks
        )
        connections[paneID] = connection
        connection.start()
    }

    func resize(paneID: UUID, size: NativeSSHTerminalSize) {
        sshLogger.debug("Resizing native SSH pane=\(paneID.uuidString, privacy: .public) size=\(size.columns)x\(size.rows)")
        connections[paneID]?.resize(size)
    }

    func stop(paneID: UUID) {
        sshLogger.info("Stopping native SSH connection pane=\(paneID.uuidString, privacy: .public)")
        connections.removeValue(forKey: paneID)?.stop()
    }
}

final class NativeSSHConnection {
    private let paneID: UUID
    private let configuration: NativeSSHConnectionConfiguration
    private let bridge: NativeSSHFileDescriptorBridge
    private let group: EventLoopGroup
    private let onError: @MainActor (SSHConnectionError) -> Void
    private let onClose: @MainActor () -> Void

    private var parentChannel: Channel?
    private var childChannel: Channel?
    private var stopped = false

    init(
        paneID: UUID,
        configuration: NativeSSHConnectionConfiguration,
        bridge: NativeSSHFileDescriptorBridge,
        size: NativeSSHTerminalSize,
        group: EventLoopGroup,
        callbacks: NativeSSHConnectionCallbacks
    ) {
        self.paneID = paneID
        self.configuration = configuration
        self.bridge = bridge
        self.group = group
        self.onError = callbacks.onError
        self.onClose = callbacks.onClose
        self.size = size
    }

    private var size: NativeSSHTerminalSize

    func start() {
        do {
            sshLogger
                .debug(
                    "Preparing SSH auth pane=\(self.paneID.uuidString, privacy: .public)"
                )
            let authDelegate = try NativeSSHAuthenticationDelegate(
                user: configuration.user,
                authentication: configuration.authentication,
                paneID: paneID,
                target: configuration.logTarget
            )
            let serverDelegate = NativeSSHServerAuthenticationDelegate(
                host: configuration.host,
                port: configuration.port,
                paneID: paneID
            )
            let bootstrap = ClientBootstrap(group: group)
                .channelInitializer { channel in
                    channel.eventLoop.makeCompletedFuture {
                        sshLogger
                            .debug(
                                "Initializing SSH pipeline pane=\(self.paneID.uuidString, privacy: .public)"
                            )
                        let ssh = NIOSSHHandler(
                            role: .client(.init(
                                userAuthDelegate: authDelegate,
                                serverAuthDelegate: serverDelegate
                            )),
                            allocator: channel.allocator,
                            inboundChildChannelInitializer: nil
                        )
                        try channel.pipeline.syncOperations.addHandler(ssh)
                        try channel.pipeline.syncOperations.addHandler(NativeSSHErrorHandler(stage: "parent", paneID: self.paneID))
                    }
                }
                .channelOption(ChannelOptions.socket(SocketOptionLevel(SOL_SOCKET), SO_REUSEADDR), value: 1)
                .channelOption(ChannelOptions.socket(SocketOptionLevel(IPPROTO_TCP), TCP_NODELAY), value: 1)

            bootstrap.connect(host: configuration.host, port: configuration.port).whenComplete { [weak self] result in
                switch result {
                case let .success(channel):
                    sshLogger
                        .info(
                            "TCP connected pane=\(self?.paneID.uuidString ?? "unknown", privacy: .public)"
                        )
                    self?.parentChannel = channel
                    self?.openSession(on: channel)
                case let .failure(error):
                    sshLogger
                        .error(
                            "TCP failed pane=\(self?.paneID.uuidString ?? "unknown", privacy: .public)"
                        )
                    self?.fail(error)
                }
            }
        } catch {
            sshLogger
                .error(
                    "SSH setup failed pane=\(self.paneID.uuidString, privacy: .public) error=\(describeError(error), privacy: .public)"
                )
            fail(error)
        }
    }

    func resize(_ size: NativeSSHTerminalSize) {
        self.size = size
        childChannel?.eventLoop.execute { [weak childChannel] in
            childChannel?.triggerUserOutboundEvent(SSHChannelRequestEvent.WindowChangeRequest(
                terminalCharacterWidth: size.columns,
                terminalRowHeight: size.rows,
                terminalPixelWidth: size.widthPixels,
                terminalPixelHeight: size.heightPixels
            ), promise: nil)
        }
    }

    func stop() {
        stopped = true
        sshLogger
            .debug(
                "Closing native SSH channels pane=\(self.paneID.uuidString, privacy: .public)"
            )
        childChannel?.close(promise: nil)
        parentChannel?.close(promise: nil)
        bridge.closeSSHSide()
    }

    private func openSession(on channel: Channel) {
        sshLogger
            .debug(
                "Opening SSH session pane=\(self.paneID.uuidString, privacy: .public)"
            )
        channel.pipeline.handler(type: NIOSSHHandler.self).flatMap { [configuration, bridge, size] sshHandler in
            let promise = channel.eventLoop.makePromise(of: Channel.self)
            sshHandler.createChannel(promise) { childChannel, channelType in
                sshLogger
                    .debug(
                        "SSH child init pane=\(self.paneID.uuidString, privacy: .public)"
                    )
                guard channelType == .session else {
                    sshLogger
                        .error(
                            "SSH child channel rejected pane=\(self.paneID.uuidString, privacy: .public)"
                        )
                    return childChannel.eventLoop.makeFailedFuture(NativeSSHConnectionFailure.invalidChannelType)
                }
                return childChannel.eventLoop.makeCompletedFuture {
                    let handler = NativeSSHShellHandler(
                        inputFD: bridge.sshReadFD,
                        outputFD: bridge.sshWriteFD,
                        command: configuration.remoteExecCommand,
                        initialInput: configuration.initialShellInput,
                        size: size,
                        paneID: self.paneID,
                        target: configuration.logTarget
                    )
                    try childChannel.pipeline.syncOperations.addHandler(handler)
                    try childChannel.pipeline.syncOperations.addHandler(NativeSSHErrorHandler(stage: "child", paneID: self.paneID))
                }
            }
            return promise.futureResult
        }.whenComplete { [weak self] result in
            switch result {
            case let .success(childChannel):
                sshLogger
                    .info(
                        "SSH session opened pane=\(self?.paneID.uuidString ?? "unknown", privacy: .public)"
                    )
                self?.childChannel = childChannel
                childChannel.closeFuture.whenComplete { [weak self] _ in
                    self?.closeFromRemote()
                }
            case let .failure(error):
                sshLogger
                    .error(
                        "SSH session failed pane=\(self?.paneID.uuidString ?? "unknown", privacy: .public)"
                    )
                self?.fail(error)
            }
        }
    }

    private func closeFromRemote() {
        guard !stopped else { return }
        stopped = true
        sshLogger
            .info(
                "SSH connection closed by remote pane=\(self.paneID.uuidString, privacy: .public)"
            )
        bridge.closeSSHSide()
        parentChannel?.close(promise: nil)
        Task { @MainActor in
            self.onClose()
        }
    }

    private func fail(_ error: Error) {
        guard !stopped else { return }
        stopped = true
        let mapped = SSHConnectionErrorMapper.map(error, host: configuration.host)
        sshLogger
            .error(
                "SSH connection failed pane=\(self.paneID.uuidString, privacy: .public) error=\(describeError(error), privacy: .public)"
            )
        bridge.closeSSHSide()
        parentChannel?.close(promise: nil)
        Task { @MainActor in
            self.onError(mapped)
        }
    }
}

extension NativeSSHConnection: @unchecked Sendable {}

final class NativeSSHShellHandler: ChannelDuplexHandler, @unchecked Sendable {
    typealias InboundIn = SSHChannelData
    typealias OutboundIn = SSHChannelData
    typealias OutboundOut = SSHChannelData

    private let inputFD: Int32
    private let outputFD: Int32
    private let command: String?
    private let initialInput: String
    private let size: NativeSSHTerminalSize
    private let paneID: UUID
    private let target: String
    private let inputQueue = DispatchQueue(label: "app.muxy.ssh.input")
    private var inputSource: DispatchSourceRead?
    private var loggedRemoteOutput = false
    private var loggedRemoteOutputWrite = false
    private var loggedLocalInput = false

    init(
        inputFD: Int32,
        outputFD: Int32,
        command: String?,
        initialInput: String,
        size: NativeSSHTerminalSize,
        paneID: UUID,
        target: String
    ) {
        self.inputFD = inputFD
        self.outputFD = outputFD
        self.command = command
        self.initialInput = initialInput
        self.size = size
        self.paneID = paneID
        self.target = target
    }

    func channelActive(context: ChannelHandlerContext) {
        sshLogger
            .info(
                "SSH shell active pane=\(self.paneID.uuidString, privacy: .public) size=\(self.size.columns)x\(self.size.rows)"
            )
        startReadingInput(context: context)
        let loopBoundContext = NIOLoopBound(context, eventLoop: context.eventLoop)
        let ptyPromise = context.eventLoop.makePromise(of: Void.self)
        context.triggerUserOutboundEvent(SSHChannelRequestEvent.PseudoTerminalRequest(
            wantReply: true,
            term: "xterm-256color",
            terminalCharacterWidth: size.columns,
            terminalRowHeight: size.rows,
            terminalPixelWidth: size.widthPixels,
            terminalPixelHeight: size.heightPixels,
            terminalModes: SSHTerminalModes([:])
        ), promise: ptyPromise)
        sshLogger.debug("SSH PTY request sent pane=\(self.paneID.uuidString, privacy: .public)")
        ptyPromise.futureResult.whenComplete { result in
            switch result {
            case .success:
                sshLogger
                    .info(
                        "SSH PTY accepted pane=\(self.paneID.uuidString, privacy: .public)"
                    )
                self.sendStartupRequest(context: loopBoundContext.value)
            case let .failure(error):
                sshLogger
                    .error(
                        "SSH PTY failed pane=\(self.paneID.uuidString, privacy: .public) error=\(describeError(error), privacy: .public)"
                    )
            }
        }
    }

    private func sendStartupRequest(context: ChannelHandlerContext) {
        if let command {
            sendExecRequest(command: command, context: context)
            return
        }
        sendShellRequest(context: context)
    }

    private func sendExecRequest(command: String, context: ChannelHandlerContext) {
        let execPromise = context.eventLoop.makePromise(of: Void.self)
        context.triggerUserOutboundEvent(SSHChannelRequestEvent.ExecRequest(command: command, wantReply: true), promise: execPromise)
        sshLogger.debug("SSH exec request sent pane=\(self.paneID.uuidString, privacy: .public)")
        execPromise.futureResult.whenComplete { result in
            switch result {
            case .success:
                sshLogger
                    .info(
                        "SSH exec accepted pane=\(self.paneID.uuidString, privacy: .public)"
                    )
            case let .failure(error):
                sshLogger
                    .error(
                        "SSH exec failed pane=\(self.paneID.uuidString, privacy: .public) error=\(describeError(error), privacy: .public)"
                    )
            }
        }
    }

    private func sendShellRequest(context: ChannelHandlerContext) {
        let loopBoundContext = NIOLoopBound(context, eventLoop: context.eventLoop)
        let shellPromise = context.eventLoop.makePromise(of: Void.self)
        context.triggerUserOutboundEvent(SSHChannelRequestEvent.ShellRequest(wantReply: true), promise: shellPromise)
        sshLogger.debug("SSH shell request sent pane=\(self.paneID.uuidString, privacy: .public)")
        shellPromise.futureResult.whenComplete { result in
            switch result {
            case .success:
                sshLogger
                    .info(
                        "SSH shell accepted pane=\(self.paneID.uuidString, privacy: .public)"
                    )
                self.writeInitialShellInput(context: loopBoundContext.value)
            case let .failure(error):
                sshLogger
                    .error(
                        "SSH shell failed pane=\(self.paneID.uuidString, privacy: .public) error=\(describeError(error), privacy: .public)"
                    )
            }
        }
    }

    private func writeInitialShellInput(context: ChannelHandlerContext) {
        guard !initialInput.isEmpty else { return }
        let bytes = Array(initialInput.utf8)
        var byteBuffer = context.channel.allocator.buffer(capacity: bytes.count)
        byteBuffer.writeBytes(bytes)
        let promise = context.eventLoop.makePromise(of: Void.self)
        context.writeAndFlush(wrapOutboundOut(SSHChannelData(type: .channel, data: .byteBuffer(byteBuffer))), promise: promise)
        promise.futureResult.whenComplete { result in
            switch result {
            case .success:
                sshLogger
                    .debug(
                        "SSH initial input sent pane=\(self.paneID.uuidString, privacy: .public) bytes=\(bytes.count)"
                    )
            case .failure:
                sshLogger
                    .error(
                        "SSH initial input failed pane=\(self.paneID.uuidString, privacy: .public)"
                    )
            }
        }
    }

    func channelRead(context: ChannelHandlerContext, data: NIOAny) {
        let data = unwrapInboundIn(data)
        guard case let .byteBuffer(buffer) = data.data else { return }
        var copy = buffer
        guard let bytes = copy.readBytes(length: copy.readableBytes), !bytes.isEmpty else { return }
        if !loggedRemoteOutput {
            loggedRemoteOutput = true
            sshLogger
                .debug(
                    "SSH remote output pane=\(self.paneID.uuidString, privacy: .public) bytes=\(bytes.count)"
                )
        }
        bytes.withUnsafeBytes { raw in
            guard let base = raw.baseAddress else { return }
            if writeFully(fd: outputFD, pointer: base, count: bytes.count) {
                if !loggedRemoteOutputWrite {
                    loggedRemoteOutputWrite = true
                    sshLogger
                        .debug(
                            "SSH remote output bridged pane=\(self.paneID.uuidString, privacy: .public) bytes=\(bytes.count)"
                        )
                }
            } else {
                sshLogger
                    .error(
                        "SSH output bridge failed pane=\(self.paneID.uuidString, privacy: .public)"
                    )
            }
        }
    }

    func channelInactive(context: ChannelHandlerContext) {
        sshLogger
            .info("SSH shell inactive pane=\(self.paneID.uuidString, privacy: .public)")
        context.fireChannelInactive()
    }

    func errorCaught(context: ChannelHandlerContext, error: Error) {
        sshLogger
            .error(
                "SSH shell error pane=\(self.paneID.uuidString, privacy: .public) error=\(describeError(error), privacy: .public)"
            )
        context.close(promise: nil)
    }

    func handlerRemoved(context: ChannelHandlerContext) {
        sshLogger
            .debug("SSH shell handler removed pane=\(self.paneID.uuidString, privacy: .public)")
        inputSource?.cancel()
        inputSource = nil
    }

    private func startReadingInput(context: ChannelHandlerContext) {
        let source = DispatchSource.makeReadSource(fileDescriptor: inputFD, queue: inputQueue)
        let loopBoundContext = NIOLoopBound(context, eventLoop: context.eventLoop)
        source.setEventHandler { [weak self] in
            guard let self else { return }
            var buffer = [UInt8](repeating: 0, count: 8192)
            let count = Darwin.read(self.inputFD, &buffer, buffer.count)
            guard count >= 1 else {
                sshLogger
                    .info(
                        "SSH input bridge closed pane=\(self.paneID.uuidString, privacy: .public)"
                    )
                loopBoundContext.eventLoop.execute {
                    loopBoundContext.value.close(promise: nil)
                }
                return
            }
            let bytes = Array(buffer.prefix(count))
            if !self.loggedLocalInput {
                self.loggedLocalInput = true
                sshLogger
                    .debug(
                        "SSH local input pane=\(self.paneID.uuidString, privacy: .public) bytes=\(bytes.count)"
                    )
            }
            loopBoundContext.eventLoop.execute {
                let context = loopBoundContext.value
                var byteBuffer = context.channel.allocator.buffer(capacity: bytes.count)
                byteBuffer.writeBytes(bytes)
                let writePromise = context.eventLoop.makePromise(of: Void.self)
                context.writeAndFlush(
                    self.wrapOutboundOut(SSHChannelData(type: .channel, data: .byteBuffer(byteBuffer))),
                    promise: writePromise
                )
                writePromise.futureResult.whenFailure { _ in
                    sshLogger
                        .error(
                            "SSH input write failed pane=\(self.paneID.uuidString, privacy: .public)"
                        )
                }
            }
        }
        source.setCancelHandler {}
        inputSource = source
        source.resume()
    }
}

final class NativeSSHAuthenticationDelegate: NIOSSHClientUserAuthenticationDelegate, @unchecked Sendable {
    private let user: String
    private var authentication: NativeSSHResolvedAuthentication?
    private let paneID: UUID
    private let target: String

    init(user: String, authentication: NativeSSHAuthentication?, paneID: UUID, target: String) throws {
        self.user = user
        self.paneID = paneID
        self.target = target
        self.authentication = try authentication.map(Self.resolve)
    }

    func nextAuthenticationType(
        availableMethods: NIOSSHAvailableUserAuthenticationMethods,
        nextChallengePromise: EventLoopPromise<NIOSSHUserAuthenticationOffer?>
    ) {
        guard let authentication else {
            sshLogger
                .error(
                    "No SSH auth offer pane=\(self.paneID.uuidString, privacy: .public)"
                )
            nextChallengePromise.succeed(nil)
            return
        }
        switch authentication {
        case let .privateKey(key) where availableMethods.contains(.publicKey):
            self.authentication = nil
            sshLogger
                .info(
                    "Offering SSH public key pane=\(self.paneID.uuidString, privacy: .public)"
                )
            nextChallengePromise.succeed(.init(username: user, serviceName: "", offer: .privateKey(.init(privateKey: key))))
        case let .password(password) where availableMethods.contains(.password):
            self.authentication = nil
            sshLogger
                .info(
                    "Offering SSH password pane=\(self.paneID.uuidString, privacy: .public)"
                )
            nextChallengePromise.succeed(.init(username: user, serviceName: "", offer: .password(.init(password: password))))
        default:
            sshLogger
                .error(
                    "SSH auth rejected pane=\(self.paneID.uuidString, privacy: .public)"
                )
            nextChallengePromise.succeed(nil)
        }
    }

    private static func resolve(_ authentication: NativeSSHAuthentication) throws -> NativeSSHResolvedAuthentication {
        switch authentication {
        case let .privateKey(path):
            try .privateKey(NativeSSHPrivateKeyLoader.load(path: path))
        case let .password(password):
            .password(password)
        }
    }
}

enum NativeSSHResolvedAuthentication {
    case privateKey(NIOSSHPrivateKey)
    case password(String)
}

enum NativeSSHPrivateKeyLoader {
    static func load(path: String) throws -> NIOSSHPrivateKey {
        let keyPath = (path as NSString).expandingTildeInPath
        let keyData = try Data(contentsOf: URL(fileURLWithPath: keyPath))
        if let text = String(data: keyData, encoding: .utf8),
           text.contains("-----BEGIN OPENSSH PRIVATE KEY-----")
        {
            return try NativeSSHOpenSSHPrivateKeyParser.parse(text)
        }
        var items: CFArray?
        var inputFormat = SecExternalFormat.formatUnknown
        var itemType = SecExternalItemType.itemTypeUnknown
        let keyParams = SecItemImportExportKeyParameters(
            version: UInt32(SEC_KEY_IMPORT_EXPORT_PARAMS_VERSION),
            flags: [],
            passphrase: nil,
            alertTitle: nil,
            alertPrompt: nil,
            accessRef: nil,
            keyUsage: nil,
            keyAttributes: nil
        )
        let status = withUnsafePointer(to: keyParams) { pointer in
            SecItemImport(keyData as CFData, nil, &inputFormat, &itemType, [], pointer, nil, &items)
        }
        guard status == errSecSuccess, let array = items as? [SecKey], let secKey = array.first else {
            throw NativeSSHConnectionFailure.privateKeyLoadFailed
        }
        guard let rawData = SecKeyCopyExternalRepresentation(secKey, nil) as? Data,
              let ed25519Key = try? Curve25519.Signing.PrivateKey(rawRepresentation: rawData)
        else {
            throw NativeSSHConnectionFailure.unsupportedPrivateKey
        }
        return NIOSSHPrivateKey(ed25519Key: ed25519Key)
    }
}

final class NativeSSHServerAuthenticationDelegate: NIOSSHClientServerAuthenticationDelegate, @unchecked Sendable {
    private let host: String
    private let port: Int
    private let paneID: UUID

    init(host: String, port: Int, paneID: UUID) {
        self.host = host
        self.port = port
        self.paneID = paneID
    }

    func validateHostKey(hostKey: NIOSSHPublicKey, validationCompletePromise: EventLoopPromise<Void>) {
        switch NativeSSHKnownHosts.validate(
            host: host,
            port: port,
            hostKey: hostKey,
            knownHosts: NativeSSHKnownHosts.loadDefaultKnownHosts()
        ) {
        case .trusted,
             .unknown:
            sshLogger
                .info(
                    "SSH host key accepted pane=\(self.paneID.uuidString, privacy: .public)"
                )
            validationCompletePromise.succeed(())
        case .changed:
            sshLogger
                .error(
                    "SSH host key changed pane=\(self.paneID.uuidString, privacy: .public)"
                )
            validationCompletePromise.fail(NativeSSHConnectionFailure.hostKeyChanged)
        }
    }
}

final class NativeSSHErrorHandler: ChannelInboundHandler, @unchecked Sendable {
    typealias InboundIn = Any
    private let stage: String
    private let paneID: UUID

    init(stage: String, paneID: UUID) {
        self.stage = stage
        self.paneID = paneID
    }

    func errorCaught(context: ChannelHandlerContext, error: Error) {
        sshLogger
            .error(
                "SSH pipeline error stage=\(self.stage, privacy: .public) pane=\(self.paneID.uuidString, privacy: .public)"
            )
        context.fireErrorCaught(error)
        context.close(promise: nil)
    }
}

enum NativeSSHConnectionFailure: Error {
    case hostKeyChanged
    case invalidChannelType
    case privateKeyLoadFailed
    case unsupportedPrivateKey
}

enum SSHConnectionErrorMapper {
    static func map(_ error: Error, host: String) -> SSHConnectionError {
        if let failure = error as? NativeSSHConnectionFailure {
            switch failure {
            case .hostKeyChanged:
                return .hostKeyChanged("The SSH host key for \(host) has changed")
            case .privateKeyLoadFailed,
                 .unsupportedPrivateKey:
                return .authFailed("Could not load SSH private key")
            case .invalidChannelType:
                return .unknown("Could not open SSH session")
            }
        }
        if let posixError = error as? POSIXError {
            switch posixError.code {
            case .ECONNREFUSED:
                return .refused(host)
            case .ETIMEDOUT:
                return .timeout(host)
            default:
                break
            }
        }
        let description = error.localizedDescription
        if description.localizedCaseInsensitiveContains("authentication") {
            return .authFailed("SSH authentication failed")
        }
        if description.localizedCaseInsensitiveContains("connection refused") {
            return .refused(host)
        }
        if description.localizedCaseInsensitiveContains("timed out") {
            return .timeout(host)
        }
        return .unknown(description)
    }
}

private func writeFully(fd: Int32, pointer: UnsafeRawPointer, count: Int) -> Bool {
    var written = 0
    while written < count {
        let result = write(fd, pointer.advanced(by: written), count - written)
        guard result > 0 else { return false }
        written += result
    }
    return true
}

private func describeError(_ error: Error) -> String {
    let description = error.localizedDescription
    let reflected = String(reflecting: error)
    guard !description.isEmpty, description != reflected else { return reflected }
    return "\(reflected) (\(description))"
}

private func channelDescription(_ channel: Channel) -> String {
    "\(ObjectIdentifier(channel as AnyObject))"
}

private func hexPreview(_ bytes: [UInt8]) -> String {
    bytes.prefix(64).map { String(format: "%02x", $0) }.joined(separator: " ")
}

private extension NativeSSHConnectionConfiguration {
    var logTarget: String {
        "\(user)@\(host):\(port)"
    }

    var authenticationLogDescription: String {
        switch authentication {
        case .privateKey:
            "privateKey"
        case .password:
            "password"
        case nil:
            "none"
        }
    }
}

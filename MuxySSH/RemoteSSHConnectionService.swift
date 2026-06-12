import CryptoKit
import Darwin
import Foundation
import NIOCore
import NIOPosix
import NIOSSH
import os
import Security

private let logger = Logger(subsystem: "app.muxy", category: "SSHConnection")

public struct SSHConnectionCallbacks {
    public let onError: @MainActor (SSHConnectionError) -> Void
    public let onClose: @MainActor () -> Void
    public let onFinished: @MainActor (UUID) -> Void

    public init(
        onError: @escaping @MainActor (SSHConnectionError) -> Void,
        onClose: @escaping @MainActor () -> Void,
        onFinished: @escaping @MainActor (UUID) -> Void
    ) {
        self.onError = onError
        self.onClose = onClose
        self.onFinished = onFinished
    }
}

@MainActor
public final class SSHConnectionService {
    public static let shared = SSHConnectionService()

    private let group = MultiThreadedEventLoopGroup(numberOfThreads: 1)
    private struct SSHConnectionSession {
        let id: UUID
        let connection: SSHConnection
        func resize(_ size: SSHTerminalSize) {
            connection.resize(size)
        }
    }

    private var connections: [UUID: SSHConnectionSession] = [:]

    private init() {}

    public func start(
        paneID: UUID,
        configuration: any SSHConnectionConfigurable,
        bridge: SSHFileDescriptorBridge,
        size: SSHTerminalSize,
        callbacks: SSHConnectionCallbacks
    ) {
        logger.info("Starting SSH for \(paneID.uuidString)")
        let sessionID = UUID()
        if let existing = connections.removeValue(forKey: paneID) {
            existing.connection.stop()
        }
        let trackedCallbacks = SSHConnectionCallbacks(
            onError: callbacks.onError,
            onClose: callbacks.onClose,
            onFinished: { [weak self] finishedPaneID in
                self?.remove(paneID: finishedPaneID, sessionID: sessionID)
                callbacks.onFinished(finishedPaneID)
            }
        )
        let connection = SSHConnection(
            paneID: paneID,
            configuration: configuration,
            bridge: bridge,
            size: size,
            group: group,
            callbacks: trackedCallbacks
        )
        connections[paneID] = SSHConnectionSession(id: sessionID, connection: connection)
        connection.start()
    }

    public func resize(paneID: UUID, size: SSHTerminalSize) {
        connections[paneID]?.resize(size)
    }

    public func stop(paneID: UUID) {
        logger.info("Stopping SSH for \(paneID.uuidString)")
        connections.removeValue(forKey: paneID)?.connection.stop()
    }

    public func remove(paneID: UUID, sessionID: UUID? = nil) {
        guard let current = connections[paneID] else { return }
        guard sessionID == nil || current.id == sessionID else { return }
        logger.debug("Removing SSH connection entry for \(paneID.uuidString)")
        connections.removeValue(forKey: paneID)
    }
}

final class SSHConnection {
    private let paneID: UUID
    private let configuration: any SSHConnectionConfigurable
    private let bridge: SSHFileDescriptorBridge
    private let group: EventLoopGroup
    private let onError: @MainActor (SSHConnectionError) -> Void
    private let onClose: @MainActor () -> Void
    private let onFinished: @MainActor (UUID) -> Void

    private var parentChannel: Channel?
    private var childChannel: Channel?
    private let lifecycleQueue = DispatchQueue(label: "app.muxy.ssh.lifecycle", attributes: .concurrent)
    private var lifecycle: SSHConnectionLifecycle = .idle

    init(
        paneID: UUID,
        configuration: any SSHConnectionConfigurable,
        bridge: SSHFileDescriptorBridge,
        size: SSHTerminalSize,
        group: EventLoopGroup,
        callbacks: SSHConnectionCallbacks
    ) {
        self.paneID = paneID
        self.configuration = configuration
        self.bridge = bridge
        self.group = group
        self.onError = callbacks.onError
        self.onClose = callbacks.onClose
        self.onFinished = callbacks.onFinished
        self.size = size
    }

    private var size: SSHTerminalSize

    func start() {
        logger.debug("Preparing SSH start for \(self.paneID.uuidString)")
        guard transition(to: .connecting, allowedFrom: [.idle, .failed, .closed]) else { return }
        do {
            let authDelegate = try SSHAuthenticationDelegate(
                user: configuration.user,
                authentication: configuration.authentication,
                paneID: paneID
            )
            let serverDelegate = SSHServerAuthenticationDelegate(
                host: configuration.host,
                port: configuration.port,
                paneID: paneID
            )
            let bootstrap = ClientBootstrap(group: group)
                .channelInitializer { channel in
                    channel.eventLoop.makeCompletedFuture {
                        logger.debug("Initializing SSH pipeline for \(self.paneID.uuidString)")
                        let ssh = NIOSSHHandler(
                            role: .client(.init(
                                userAuthDelegate: authDelegate,
                                serverAuthDelegate: serverDelegate
                            )),
                            allocator: channel.allocator,
                            inboundChildChannelInitializer: nil
                        )
                        try channel.pipeline.syncOperations.addHandler(ssh)
                        try channel.pipeline.syncOperations.addHandler(SSHErrorHandler(stage: "parent", paneID: self.paneID))
                    }
                }
                .channelOption(ChannelOptions.socket(SocketOptionLevel(SOL_SOCKET), SO_REUSEADDR), value: 1)
                .channelOption(ChannelOptions.socket(SocketOptionLevel(IPPROTO_TCP), TCP_NODELAY), value: 1)

            bootstrap.connect(host: configuration.host, port: configuration.port).whenComplete { [weak self] result in
                switch result {
                case let .success(channel):
                    logger.info("TCP connected for \(self?.paneID.uuidString ?? "unknown")")
                    guard self?.currentState() == .connecting else {
                        channel.close(promise: nil)
                        return
                    }
                    self?.parentChannel = channel
                    self?.openSession(on: channel)
                case let .failure(error):
                    logger.error("TCP connect failed for \(self?.paneID.uuidString ?? "unknown"): \(error)")
                    self?.fail(error)
                }
            }
        } catch {
            logger.error("SSH setup failed for \(self.paneID.uuidString): \(error)")
            fail(error)
        }
    }

    func resize(_ size: SSHTerminalSize) {
        self.size = size
        guard currentState() == .running else { return }
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
        if transition(to: .stopping, allowedFrom: [.connecting, .running]) {
            logger.debug("Stopping SSH for \(self.paneID.uuidString)")
            tearDown()
            _ = transition(to: .closed, allowedFrom: [.stopping])
            return
        }

        if currentState() == .stopping { return }
    }

    private func openSession(on channel: Channel) {
        logger.debug("Opening SSH session for \(self.paneID.uuidString)")
        channel.pipeline.handler(type: NIOSSHHandler.self).flatMap { [configuration, bridge, size] sshHandler in
            let promise = channel.eventLoop.makePromise(of: Channel.self)
            sshHandler.createChannel(promise) { childChannel, channelType in
                logger.debug("SSH child init for \(self.paneID.uuidString)")
                guard channelType == .session else {
                    logger.error("SSH child channel rejected for \(self.paneID.uuidString)")
                    return childChannel.eventLoop.makeFailedFuture(SSHConnectionFailure.invalidChannelType)
                }
                return childChannel.eventLoop.makeCompletedFuture {
                    let handler = SSHShellHandler(
                        inputFD: bridge.sshReadFD,
                        outputFD: bridge.sshWriteFD,
                        command: configuration.remoteExecCommand,
                        initialInput: configuration.initialShellInput,
                        size: size,
                        paneID: self.paneID
                    )
                    try childChannel.pipeline.syncOperations.addHandler(handler)
                    try childChannel.pipeline.syncOperations.addHandler(SSHErrorHandler(stage: "child", paneID: self.paneID))
                }
            }
            return promise.futureResult
        }.whenComplete { [weak self] result in
            switch result {
            case let .success(childChannel):
                guard self?.transition(to: .running, allowedFrom: [.connecting]) == true else {
                    childChannel.close(promise: nil)
                    return
                }
                logger.info("SSH session opened for \(self?.paneID.uuidString ?? "unknown")")
                self?.childChannel = childChannel
                childChannel.closeFuture.whenComplete { [weak self] _ in
                    self?.closeFromRemote()
                }
            case let .failure(error):
                logger.error("SSH session failed for \(self?.paneID.uuidString ?? "unknown"): \(error)")
                self?.fail(error)
            }
        }
    }

    private func closeFromRemote() {
        guard transition(to: .closed, allowedFrom: [.running]) else { return }
        logger.info("SSH connection closed by remote for \(self.paneID.uuidString)")
        tearDown()
        Task { @MainActor in
            self.onClose()
            self.onFinished(self.paneID)
        }
    }

    private func fail(_ error: Error) {
        guard transition(to: .failed, allowedFrom: [.connecting, .running]) else { return }
        let mapped = SSHConnectionErrorMapper.map(error, host: configuration.host)
        logger.error("SSH connection failed for \(self.paneID.uuidString): \(error)")
        tearDown()
        Task { @MainActor in
            self.onError(mapped)
            self.onFinished(self.paneID)
        }
    }

    private func tearDown(closeBridge: Bool = true) {
        childChannel?.close(promise: nil)
        parentChannel?.close(promise: nil)
        if closeBridge {
            bridge.closeSSHSide()
        }
        childChannel = nil
        parentChannel = nil
    }

    private func transition(
        to nextState: SSHConnectionLifecycle,
        allowedFrom allowed: Set<SSHConnectionLifecycle>
    ) -> Bool {
        lifecycleQueue.sync(flags: .barrier) {
            guard allowed.contains(lifecycle) else { return false }
            lifecycle = nextState
            return true
        }
    }

    private func currentState() -> SSHConnectionLifecycle {
        lifecycleQueue.sync { lifecycle }
    }
}

extension SSHConnection: @unchecked Sendable {}

private enum SSHConnectionLifecycle {
    case idle
    case connecting
    case running
    case stopping
    case failed
    case closed
}

final class SSHShellHandler: ChannelDuplexHandler, @unchecked Sendable {
    typealias InboundIn = SSHChannelData
    typealias OutboundIn = SSHChannelData
    typealias OutboundOut = SSHChannelData

    private let inputFD: Int32
    private let outputFD: Int32
    private let command: String?
    private let initialInput: String
    private let size: SSHTerminalSize
    private let paneID: UUID
    private let inputQueue = DispatchQueue(label: "app.muxy.ssh.input")
    private var inputSource: DispatchSourceRead?

    init(
        inputFD: Int32,
        outputFD: Int32,
        command: String?,
        initialInput: String,
        size: SSHTerminalSize,
        paneID: UUID
    ) {
        self.inputFD = inputFD
        self.outputFD = outputFD
        self.command = command
        self.initialInput = initialInput
        self.size = size
        self.paneID = paneID
    }

    func channelActive(context: ChannelHandlerContext) {
        logger.info("SSH shell active for \(self.paneID.uuidString)")
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
        ptyPromise.futureResult.whenComplete { result in
            switch result {
            case .success:
                logger.info("SSH PTY accepted for \(self.paneID.uuidString)")
                self.sendStartupRequest(context: loopBoundContext.value)
            case let .failure(error):
                logger.error("SSH PTY failed for \(self.paneID.uuidString): \(error)")
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
        execPromise.futureResult.whenComplete { result in
            switch result {
            case .success:
                logger.info("SSH exec accepted for \(self.paneID.uuidString)")
            case let .failure(error):
                logger.error("SSH exec failed for \(self.paneID.uuidString): \(error)")
            }
        }
    }

    private func sendShellRequest(context: ChannelHandlerContext) {
        let loopBoundContext = NIOLoopBound(context, eventLoop: context.eventLoop)
        let shellPromise = context.eventLoop.makePromise(of: Void.self)
        context.triggerUserOutboundEvent(SSHChannelRequestEvent.ShellRequest(wantReply: true), promise: shellPromise)
        shellPromise.futureResult.whenComplete { result in
            switch result {
            case .success:
                logger.info("SSH shell accepted for \(self.paneID.uuidString)")
                self.writeInitialShellInput(context: loopBoundContext.value)
            case let .failure(error):
                logger.error("SSH shell failed for \(self.paneID.uuidString): \(error)")
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
                break
            case .failure:
                logger.error("SSH initial input failed for \(self.paneID.uuidString)")
            }
        }
    }

    func channelRead(context: ChannelHandlerContext, data: NIOAny) {
        let data = unwrapInboundIn(data)
        guard case let .byteBuffer(buffer) = data.data else { return }
        var copy = buffer
        guard let bytes = copy.readBytes(length: copy.readableBytes), !bytes.isEmpty else { return }
        bytes.withUnsafeBytes { raw in
            guard let base = raw.baseAddress else { return }
            if writeFully(fd: outputFD, pointer: base, count: bytes.count) {
            } else {
                logger.error("SSH output bridge write failed for \(self.paneID.uuidString)")
            }
        }
    }

    func channelInactive(context: ChannelHandlerContext) {
        logger.info("SSH shell inactive for \(self.paneID.uuidString)")
        context.fireChannelInactive()
    }

    func errorCaught(context: ChannelHandlerContext, error: Error) {
        logger.error("SSH shell error for \(self.paneID.uuidString): \(error)")
        context.close(promise: nil)
    }

    func handlerRemoved(context: ChannelHandlerContext) {
        logger.debug("SSH shell handler removed for \(self.paneID.uuidString)")
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
            if count > 0 {
                let bytes = Array(buffer.prefix(count))
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
                        logger.error("SSH input write failed for \(self.paneID.uuidString)")
                    }
                }
                return
            }

            guard count != -1 else {
                logger.info("SSH input bridge closed for \(self.paneID.uuidString)")
                loopBoundContext.eventLoop.execute {
                    loopBoundContext.value.close(promise: nil)
                }
                return
            }

            let readError = errno
            guard readError == EAGAIN || readError == EWOULDBLOCK || readError == EINTR else {
                logger.error("SSH input bridge read failed for \(self.paneID.uuidString): \(readError)")
                loopBoundContext.eventLoop.execute {
                    loopBoundContext.value.close(promise: nil)
                }
                return
            }
        }
        source.setCancelHandler {}
        inputSource = source
        source.resume()
    }
}

final class SSHAuthenticationDelegate: NIOSSHClientUserAuthenticationDelegate, @unchecked Sendable {
    private let user: String
    private var authentication: SSHResolvedAuthentication?
    private let paneID: UUID

    init(user: String, authentication: SSHAuthentication?, paneID: UUID) throws {
        self.user = user
        self.paneID = paneID
        self.authentication = try authentication.map(Self.resolve)
    }

    func nextAuthenticationType(
        availableMethods: NIOSSHAvailableUserAuthenticationMethods,
        nextChallengePromise: EventLoopPromise<NIOSSHUserAuthenticationOffer?>
    ) {
        guard let authentication else {
            logger.error("No SSH auth offer for \(self.paneID.uuidString)")
            nextChallengePromise.succeed(nil)
            return
        }
        switch authentication {
        case let .privateKey(key) where availableMethods.contains(.publicKey):
            self.authentication = nil
            logger.info("Offering SSH public key for \(self.paneID.uuidString)")
            nextChallengePromise.succeed(.init(username: user, serviceName: "", offer: .privateKey(.init(privateKey: key))))
        case let .password(password) where availableMethods.contains(.password):
            self.authentication = nil
            logger.info("Offering SSH password for \(self.paneID.uuidString)")
            nextChallengePromise.succeed(.init(username: user, serviceName: "", offer: .password(.init(password: password))))
        default:
            logger.error("SSH auth rejected for \(self.paneID.uuidString)")
            nextChallengePromise.succeed(nil)
        }
    }

    private static func resolve(_ authentication: SSHAuthentication) throws -> SSHResolvedAuthentication {
        switch authentication {
        case let .privateKey(path):
            try .privateKey(SSHPrivateKeyLoader.load(path: path))
        case let .password(password):
            .password(password)
        }
    }
}

enum SSHResolvedAuthentication {
    case privateKey(NIOSSHPrivateKey)
    case password(String)
}

public enum SSHPrivateKeyLoader {
    public static func load(path: String) throws -> NIOSSHPrivateKey {
        let keyPath = (path as NSString).expandingTildeInPath
        let keyData = try Data(contentsOf: URL(fileURLWithPath: keyPath))
        if let text = String(data: keyData, encoding: .utf8),
           text.contains("-----BEGIN OPENSSH PRIVATE KEY-----")
        {
            return try SSHOpenSSHPrivateKeyParser.parse(text)
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
            throw SSHConnectionFailure.privateKeyLoadFailed
        }
        guard let rawData = SecKeyCopyExternalRepresentation(secKey, nil) as? Data,
              let ed25519Key = try? Curve25519.Signing.PrivateKey(rawRepresentation: rawData)
        else {
            throw SSHConnectionFailure.unsupportedKeyType
        }
        return NIOSSHPrivateKey(ed25519Key: ed25519Key)
    }
}

final class SSHServerAuthenticationDelegate: NIOSSHClientServerAuthenticationDelegate, @unchecked Sendable {
    private let host: String
    private let port: Int
    private let paneID: UUID

    init(host: String, port: Int, paneID: UUID) {
        self.host = host
        self.port = port
        self.paneID = paneID
    }

    func validateHostKey(hostKey: NIOSSHPublicKey, validationCompletePromise: EventLoopPromise<Void>) {
        switch SSHKnownHosts.validate(
            host: host,
            port: port,
            hostKey: hostKey,
            knownHosts: SSHKnownHosts.loadDefaultKnownHosts()
        ) {
        case .trusted:
            logger.info("SSH host key accepted for \(self.paneID.uuidString)")
            validationCompletePromise.succeed(())
        case .unknown:
            logger.error("SSH host key unknown for \(self.paneID.uuidString)")
            validationCompletePromise.fail(SSHConnectionFailure.unknownHostKey)
        case .changed:
            logger.error("SSH host key changed for \(self.paneID.uuidString)")
            validationCompletePromise.fail(SSHConnectionFailure.hostKeyChanged)
        }
    }
}

final class SSHErrorHandler: ChannelInboundHandler, @unchecked Sendable {
    typealias InboundIn = Any
    private let stage: String
    private let paneID: UUID

    init(stage: String, paneID: UUID) {
        self.stage = stage
        self.paneID = paneID
    }

    func errorCaught(context: ChannelHandlerContext, error: Error) {
        logger.error("SSH pipeline error in \(self.stage) stage for \(self.paneID.uuidString)")
        context.fireErrorCaught(error)
        context.close(promise: nil)
    }
}

public enum SSHConnectionFailure: Error {
    case hostKeyChanged
    case unknownHostKey
    case invalidChannelType
    case privateKeyLoadFailed
    case unsupportedPrivateKey
    case encryptedPrivateKey
    case unsupportedKeyType
}

public enum SSHConnectionErrorMapper {
    public static func map(_ error: Error, host: String) -> SSHConnectionError {
        if let failure = error as? SSHConnectionFailure {
            switch failure {
            case .hostKeyChanged:
                return .hostKeyChanged("The SSH host key for \(host) has changed")
            case .unknownHostKey:
                return .unknownHostKey(host)
            case .privateKeyLoadFailed,
                 .unsupportedPrivateKey:
                return .authFailed("Could not load SSH private key")
            case .encryptedPrivateKey:
                return .authFailed("Encrypted SSH private keys are not supported yet. Use an unencrypted key.")
            case .unsupportedKeyType:
                return .authFailed("Only Ed25519 private keys are supported for SSH.")
            case .invalidChannelType:
                return .unknown("Could not open SSH session")
            }
        }
        if let sshError = error as? NIOSSHError {
            return mapNIOSSHError(sshError, host: host)
        }
        if let posixError = error as? POSIXError {
            return mapPOSIXError(posixError, host: host)
        }
        if let channelError = error as? ChannelError {
            return mapChannelError(channelError, host: host)
        }
        return fallbackMap(error, host: host)
    }

    private static func mapNIOSSHError(_ error: NIOSSHError, host: String) -> SSHConnectionError {
        switch error.type {
        case .invalidUserAuthSignature:
            .authFailed("SSH authentication failed")
        case .channelSetupRejected:
            .unknown("SSH channel was rejected by \(host)")
        case .keyExchangeNegotiationFailure:
            .unknown("Could not agree on encryption with \(host)")
        case .unsupportedVersion:
            .unknown("SSH version not supported by \(host)")
        case .tcpShutdown:
            .unknown("Connection to \(host) was closed")
        default:
            .unknown(error.localizedDescription)
        }
    }

    private static func mapPOSIXError(_ error: POSIXError, host: String) -> SSHConnectionError {
        switch error.code {
        case .ECONNREFUSED:
            .refused(host)
        case .ETIMEDOUT:
            .timeout(host)
        case .EHOSTUNREACH,
             .ENETUNREACH:
            .unknown("Cannot reach \(host)")
        case .ECONNABORTED:
            .unknown("Connection to \(host) was aborted")
        case .ENETDOWN:
            .unknown("Network is unavailable")
        default:
            .unknown(error.localizedDescription)
        }
    }

    private static func mapChannelError(_ error: ChannelError, host: String) -> SSHConnectionError {
        switch error {
        case .connectTimeout:
            .timeout(host)
        case .ioOnClosedChannel:
            .unknown("SSH connection to \(host) was closed unexpectedly")
        case .eof:
            .unknown("Connection to \(host) was closed by remote peer")
        default:
            .unknown(error.localizedDescription)
        }
    }

    private static func fallbackMap(_ error: Error, host: String) -> SSHConnectionError {
        let description = error.localizedDescription
        if description.localizedCaseInsensitiveContains("authentication") {
            return .authFailed("SSH authentication failed")
        }
        if description.localizedCaseInsensitiveContains("host key") {
            return .hostKeyChanged("SSH host key verification failed for \(host)")
        }
        return .unknown(description)
    }
}

private func writeFully(fd: Int32, pointer: UnsafeRawPointer, count: Int) -> Bool {
    var written = 0
    while written < count {
        let result = write(fd, pointer.advanced(by: written), count - written)
        guard result != -1 else {
            let writeError = errno
            if writeError == EAGAIN || writeError == EWOULDBLOCK || writeError == EINTR {
                if !waitUntilWritable(fd: fd) { return false }
                continue
            }
            return false
        }
        guard result > 0 else { return false }
        written += result
    }
    return true
}

private func waitUntilWritable(fd: Int32) -> Bool {
    var pollDescriptor = pollfd(fd: fd, events: Int16(POLLOUT), revents: 0)
    while true {
        let result = poll(&pollDescriptor, 1, 25)
        if result > 0 {
            let revents = pollDescriptor.revents
            if revents & Int16(POLLHUP) != 0 { return false }
            if revents & Int16(POLLERR) != 0 { return false }
            if revents & Int16(POLLNVAL) != 0 { return false }
            return (revents & Int16(POLLOUT)) != 0
        }
        if result == 0 { return false }
        if errno == EINTR { continue }
        return false
    }
}

import Foundation
import NIOCore
import NIOPosix
import NIOSSH
import os

private let execLogger = Logger(subsystem: "app.muxy", category: "SSHExecService")
private let execStatusMarker = "__MUXY_STATUS__:"

public struct SSHExecResult: Sendable {
    public let status: Int32
    public let stdout: String
    public let stdoutData: Data
    public let stderr: String

    public init(status: Int32, stdout: String, stdoutData: Data, stderr: String) {
        self.status = status
        self.stdout = stdout
        self.stdoutData = stdoutData
        self.stderr = stderr
    }
}

public final class SSHExecService: @unchecked Sendable {
    public static let shared = SSHExecService()

    private let group = MultiThreadedEventLoopGroup(numberOfThreads: 1)

    public init() {}

    public func run(
        configuration: any SSHConnectionConfigurable,
        command: String,
        stdinData: Data? = nil,
        timeout: TimeInterval = 60
    ) async throws -> SSHExecResult {
        try await withCheckedThrowingContinuation { continuation in
            let runner = SSHExecRunner(
                configuration: configuration,
                command: wrapped(command),
                stdinData: stdinData,
                timeout: timeout,
                group: group
            ) { result in
                continuation.resume(with: result)
            }
            runner.start()
        }
    }

    private func wrapped(_ command: String) -> String {
        "(\(command)); muxy_status=$?; printf '\\n\(execStatusMarker)%s\\n' \"$muxy_status\""
    }
}

private final class SSHExecRunner: @unchecked Sendable {
    private let configuration: any SSHConnectionConfigurable
    private let command: String
    private let stdinData: Data?
    private let timeout: TimeInterval
    private let group: EventLoopGroup
    private let completion: (Result<SSHExecResult, Error>) -> Void

    private var timer: DispatchSourceTimer?
    private var parentChannel: Channel?
    private var childChannel: Channel?
    private var finished = false
    private let id = UUID()

    init(
        configuration: any SSHConnectionConfigurable,
        command: String,
        stdinData: Data?,
        timeout: TimeInterval,
        group: EventLoopGroup,
        completion: @escaping (Result<SSHExecResult, Error>) -> Void
    ) {
        self.configuration = configuration
        self.command = command
        self.stdinData = stdinData
        self.timeout = timeout
        self.group = group
        self.completion = completion
    }

    func start() {
        do {
            let authDelegate = try SSHAuthenticationDelegate(
                user: configuration.user,
                authentication: configuration.authentication,
                paneID: id
            )
            let serverDelegate = SSHServerAuthenticationDelegate(
                host: configuration.host,
                port: configuration.port,
                paneID: id
            )
            let bootstrap = ClientBootstrap(group: group)
                .channelInitializer { channel in
                    channel.eventLoop.makeCompletedFuture {
                        let ssh = NIOSSHHandler(
                            role: .client(.init(
                                userAuthDelegate: authDelegate,
                                serverAuthDelegate: serverDelegate
                            )),
                            allocator: channel.allocator,
                            inboundChildChannelInitializer: nil
                        )
                        try channel.pipeline.syncOperations.addHandler(ssh)
                        try channel.pipeline.syncOperations.addHandler(SSHErrorHandler(stage: "exec-parent", paneID: self.id))
                    }
                }
                .channelOption(ChannelOptions.socket(SocketOptionLevel(SOL_SOCKET), SO_REUSEADDR), value: 1)
                .channelOption(ChannelOptions.socket(SocketOptionLevel(IPPROTO_TCP), TCP_NODELAY), value: 1)

            let timer = DispatchSource.makeTimerSource()
            timer.schedule(deadline: .now() + .milliseconds(Int(timeout * 1000)))
            timer.setEventHandler { [weak self] in
                self?.finish(.failure(POSIXError(.ETIMEDOUT)))
            }
            self.timer = timer
            timer.resume()

            bootstrap.connect(host: configuration.host, port: configuration.port).whenComplete { [weak self] result in
                guard let self else { return }
                switch result {
                case let .success(channel):
                    self.parentChannel = channel
                    self.openSession(on: channel)
                case let .failure(error):
                    self.finish(.failure(error))
                }
            }
        } catch {
            finish(.failure(error))
        }
    }

    private func openSession(on channel: Channel) {
        channel.pipeline.handler(type: NIOSSHHandler.self).flatMap { [self] sshHandler in
            let promise = channel.eventLoop.makePromise(of: Channel.self)
            sshHandler.createChannel(promise) { childChannel, channelType in
                guard channelType == .session else {
                    return childChannel.eventLoop.makeFailedFuture(SSHConnectionFailure.invalidChannelType)
                }
                return childChannel.eventLoop.makeCompletedFuture {
                    let handler = SSHExecHandler(
                        command: self.command,
                        stdinData: self.stdinData,
                        paneID: self.id
                    ) { result in
                        self.finish(result)
                    }
                    try childChannel.pipeline.syncOperations.addHandler(handler)
                    try childChannel.pipeline.syncOperations.addHandler(SSHErrorHandler(stage: "exec-child", paneID: self.id))
                }
            }
            return promise.futureResult
        }.whenComplete { [weak self] result in
            guard let self else { return }
            switch result {
            case let .success(channel):
                self.childChannel = channel
            case let .failure(error):
                self.finish(.failure(error))
            }
        }
    }

    private func finish(_ result: Result<SSHExecResult, Error>) {
        guard !finished else { return }
        finished = true
        timer?.cancel()
        timer = nil
        childChannel?.close(promise: nil)
        parentChannel?.close(promise: nil)
        childChannel = nil
        parentChannel = nil

        switch result {
        case .success:
            completion(result)
        case let .failure(error as SSHConnectionError):
            completion(.failure(error))
        case let .failure(error as NIOSSHError):
            completion(.failure(SSHConnectionErrorMapper.map(error, host: configuration.host)))
        case let .failure(error as POSIXError):
            completion(.failure(SSHConnectionErrorMapper.map(error, host: configuration.host)))
        case let .failure(error as ChannelError):
            completion(.failure(SSHConnectionErrorMapper.map(error, host: configuration.host)))
        case let .failure(error):
            execLogger.error("SSH exec failed for \(self.configuration.host, privacy: .public): \(error.localizedDescription, privacy: .public)")
            completion(.failure(error))
        }
    }
}

private final class SSHExecHandler: ChannelDuplexHandler, @unchecked Sendable {
    typealias InboundIn = SSHChannelData
    typealias OutboundIn = SSHChannelData
    typealias OutboundOut = SSHChannelData

    private let command: String
    private let stdinData: Data?
    private let paneID: UUID
    private let completion: (Result<SSHExecResult, Error>) -> Void

    private var stdout = Data()
    private var stderr = Data()
    private var completed = false
    private var context: ChannelHandlerContext?

    init(
        command: String,
        stdinData: Data?,
        paneID: UUID,
        completion: @escaping (Result<SSHExecResult, Error>) -> Void
    ) {
        self.command = command
        self.stdinData = stdinData
        self.paneID = paneID
        self.completion = completion
    }

    func channelActive(context: ChannelHandlerContext) {
        self.context = context
        let promise = context.eventLoop.makePromise(of: Void.self)
        context.triggerUserOutboundEvent(
            SSHChannelRequestEvent.ExecRequest(command: command, wantReply: true),
            promise: promise
        )
        promise.futureResult.whenComplete { [weak self] result in
            guard let self else { return }
            switch result {
            case .success:
                self.writeInputIfNeeded(context: context)
            case let .failure(error):
                self.completeOnce(.failure(error))
            }
        }
    }

    func channelRead(context: ChannelHandlerContext, data: NIOAny) {
        let chunk = unwrapInboundIn(data)
        guard case let .byteBuffer(buffer) = chunk.data else { return }
        var copy = buffer
        guard let bytes = copy.readBytes(length: copy.readableBytes), !bytes.isEmpty else { return }
        switch chunk.type {
        case .channel:
            stdout.append(contentsOf: bytes)
        case .stdErr:
            stderr.append(contentsOf: bytes)
        default:
            return
        }
    }

    func channelInactive(context: ChannelHandlerContext) {
        completeOnce(parsedResult())
        context.fireChannelInactive()
    }

    func errorCaught(context: ChannelHandlerContext, error: Error) {
        completeOnce(.failure(error))
        context.close(promise: nil)
    }

    private func writeInputIfNeeded(context: ChannelHandlerContext) {
        guard let stdinData, !stdinData.isEmpty else {
            context.close(mode: .output, promise: nil)
            return
        }
        var buffer = context.channel.allocator.buffer(capacity: stdinData.count)
        buffer.writeBytes(stdinData)
        context.writeAndFlush(
            wrapOutboundOut(SSHChannelData(type: .channel, data: .byteBuffer(buffer)))
        ).whenComplete { _ in
            context.close(mode: .output, promise: nil)
        }
    }

    private func parsedResult() -> Result<SSHExecResult, Error> {
        let parsed = parseStatus(from: stdout)
        return .success(SSHExecResult(
            status: parsed.status,
            stdout: String(decoding: parsed.stdoutData, as: UTF8.self),
            stdoutData: parsed.stdoutData,
            stderr: String(decoding: stderr, as: UTF8.self)
        ))
    }

    private func parseStatus(from data: Data) -> (status: Int32, stdoutData: Data) {
        let text = String(decoding: data, as: UTF8.self)
        guard let range = text.range(of: execStatusMarker, options: .backwards) else {
            return (1, data)
        }
        let statusText = text[range.upperBound...].trimmingCharacters(in: .whitespacesAndNewlines)
        let status = Int32(statusText) ?? 1
        let payload = String(text[..<range.lowerBound]).trimmingCharacters(in: .newlines)
        return (status, Data(payload.utf8))
    }

    private func completeOnce(_ result: Result<SSHExecResult, Error>) {
        guard !completed else { return }
        completed = true
        completion(result)
    }
}

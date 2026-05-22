import Foundation

enum BrowserWebViewCommand {
    case navigate(URL)
    case reload
    case stop
    case back
    case forward
    case setZoom(Double)
    case scrollTo(Double)
    case setInspectorMode(BrowserInspectorState.Mode)
    case applyStyleOverrides([StyleOverride])
}

@MainActor
final class BrowserCommandBus {
    let stream: AsyncStream<BrowserWebViewCommand>
    private let continuation: AsyncStream<BrowserWebViewCommand>.Continuation

    init() {
        let (stream, continuation) = AsyncStream.makeStream(of: BrowserWebViewCommand.self)
        self.stream = stream
        self.continuation = continuation
    }

    func send(_ command: BrowserWebViewCommand) {
        continuation.yield(command)
    }

    deinit {
        continuation.finish()
    }
}

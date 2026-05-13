import AppKit
import Carbon.HIToolbox
import SwiftUI

struct VoiceRecordingPanel: View {
    @Bindable var state: VoiceRecordingState
    let autoSend: Bool
    @State private var isFocused = false
    @State private var pulse = false

    private static let levelBarSpacing: CGFloat = 3

    var body: some View {
        VStack(spacing: UIMetrics.spacing3) {
            transcriptChip
            mainPanel
            keyboardHints
        }
        .padding(.bottom, UIMetrics.scaled(48))
        .background(VoicePanelFocusTrap(
            isFocused: $isFocused,
            onFinish: { state.finish(autoSend: autoSend) },
            onCancel: { state.cancel() },
            onTogglePause: { state.togglePause() }
        ))
        .onAppear { pulse = true }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Voice recording")
    }

    private var mainPanel: some View {
        HStack(spacing: UIMetrics.spacing5) {
            statusDot
            timerLabel
            levelMeter
                .frame(maxWidth: .infinity)
            controlButtons
        }
        .padding(.horizontal, UIMetrics.spacing6)
        .padding(.vertical, UIMetrics.spacing5)
        .background(MuxyTheme.bg, in: RoundedRectangle(cornerRadius: UIMetrics.radiusLG))
        .overlay(
            RoundedRectangle(cornerRadius: UIMetrics.radiusLG)
                .stroke(isFocused ? MuxyTheme.accent.opacity(0.6) : MuxyTheme.border, lineWidth: 1)
        )
        .shadow(color: Color.black.opacity(0.22), radius: 14, x: 0, y: -6)
        .frame(width: UIMetrics.scaled(280))
    }

    private var transcriptChip: some View {
        secondaryRow
            .padding(.horizontal, UIMetrics.spacing5)
            .padding(.vertical, UIMetrics.spacing3)
            .frame(width: UIMetrics.scaled(280), alignment: .leading)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
    }

    @ViewBuilder
    private var secondaryRow: some View {
        if let message = state.errorMessage {
            Text(message)
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.diffRemoveFg)
                .lineLimit(2)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
        } else if !state.recorder.transcript.isEmpty {
            Text(state.recorder.transcript)
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
                .lineLimit(4)
                .truncationMode(.head)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            Text("Listening...")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgDim)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var statusDot: some View {
        Circle()
            .fill(state.recorder.isPaused ? MuxyTheme.warning : MuxyTheme.diffRemoveFg)
            .frame(width: UIMetrics.iconSM, height: UIMetrics.iconSM)
            .opacity(state.recorder.isPaused ? 1.0 : (pulse ? 1.0 : 0.4))
            .animation(
                state.recorder.isPaused
                    ? .default
                    : .easeInOut(duration: 0.8).repeatForever(autoreverses: true),
                value: pulse
            )
    }

    private var timerLabel: some View {
        Text(Self.formatElapsed(state.recorder.elapsed))
            .font(.system(size: UIMetrics.fontEmphasis, weight: .medium, design: .monospaced))
            .foregroundStyle(MuxyTheme.fg)
            .frame(width: UIMetrics.scaled(60), alignment: .leading)
    }

    private var levelMeter: some View {
        GeometryReader { geometry in
            let barCount = max(8, Int(geometry.size.width / UIMetrics.scaled(6)))
            HStack(spacing: UIMetrics.scaled(Self.levelBarSpacing)) {
                ForEach(0 ..< barCount, id: \.self) { index in
                    let threshold = Float(index + 1) / Float(barCount)
                    let active = !state.recorder.isPaused && state.recorder.level >= threshold
                    Capsule()
                        .fill(active ? MuxyTheme.accent : MuxyTheme.surface)
                        .frame(maxWidth: .infinity, maxHeight: barHeight(at: index, of: barCount))
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height)
        }
        .frame(height: UIMetrics.scaled(24))
        .accessibilityHidden(true)
    }

    private func barHeight(at index: Int, of total: Int) -> CGFloat {
        let normalized = Double(index) / Double(max(1, total - 1))
        let curve = 0.5 + 0.5 * sin(normalized * .pi)
        return UIMetrics.scaled(10 + 14 * curve)
    }

    private var controlButtons: some View {
        HStack(spacing: UIMetrics.spacing3) {
            iconButton(systemName: "xmark", tint: MuxyTheme.fgMuted, accessibility: "Cancel") {
                state.cancel()
            }
            iconButton(
                systemName: state.recorder.isPaused ? "play.fill" : "pause.fill",
                tint: MuxyTheme.fg,
                accessibility: state.recorder.isPaused ? "Resume" : "Pause"
            ) {
                state.togglePause()
            }
            iconButton(
                systemName: "circle.fill",
                tint: MuxyTheme.diffRemoveFg,
                accessibility: "Send"
            ) {
                state.finish(autoSend: autoSend)
            }
        }
    }

    private var keyboardHints: some View {
        HStack(spacing: UIMetrics.spacing5) {
            hint(key: "⎋", label: "Cancel")
            hint(key: "Space", label: state.recorder.isPaused ? "Resume" : "Pause")
            hint(key: "⏎", label: autoSend ? "Send" : "Insert")
        }
        .font(.system(size: UIMetrics.fontCaption))
        .foregroundStyle(MuxyTheme.fgDim)
        .padding(.horizontal, UIMetrics.spacing4)
        .padding(.vertical, UIMetrics.scaled(3))
        .background(MuxyTheme.bg.opacity(0.85), in: Capsule())
        .overlay(Capsule().stroke(MuxyTheme.border, lineWidth: 1))
        .opacity(isFocused ? 1 : 0.55)
    }

    private func hint(key: String, label: String) -> some View {
        HStack(spacing: UIMetrics.spacing1) {
            Text(key)
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text(label)
        }
    }

    private func iconButton(
        systemName: String,
        tint: Color,
        accessibility: String,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(tint)
                .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(accessibility)
    }

    nonisolated static func formatElapsed(_ elapsed: TimeInterval) -> String {
        let total = max(0, Int(elapsed))
        return Duration.seconds(total).formatted(.time(pattern: .minuteSecond(padMinuteToLength: 2)))
    }
}

private struct VoicePanelFocusTrap: NSViewRepresentable {
    @Binding var isFocused: Bool
    let onFinish: () -> Void
    let onCancel: () -> Void
    let onTogglePause: () -> Void

    func makeNSView(context: Context) -> VoicePanelFocusTrapView {
        let view = VoicePanelFocusTrapView()
        view.onFinish = onFinish
        view.onCancel = onCancel
        view.onTogglePause = onTogglePause
        view.onFocusChange = { focused in
            DispatchQueue.main.async { isFocused = focused }
        }
        DispatchQueue.main.async {
            view.window?.makeFirstResponder(view)
        }
        return view
    }

    func updateNSView(_ nsView: VoicePanelFocusTrapView, context _: Context) {
        nsView.onFinish = onFinish
        nsView.onCancel = onCancel
        nsView.onTogglePause = onTogglePause
    }
}

final class VoicePanelFocusTrapView: NSView {
    var onFinish: (() -> Void)?
    var onCancel: (() -> Void)?
    var onTogglePause: (() -> Void)?
    var onFocusChange: ((Bool) -> Void)?

    override var acceptsFirstResponder: Bool { true }

    override func becomeFirstResponder() -> Bool {
        let result = super.becomeFirstResponder()
        if result { onFocusChange?(true) }
        return result
    }

    override func resignFirstResponder() -> Bool {
        let result = super.resignFirstResponder()
        if result { onFocusChange?(false) }
        return result
    }

    override func keyDown(with event: NSEvent) {
        switch Int(event.keyCode) {
        case kVK_Return,
             kVK_ANSI_KeypadEnter:
            onFinish?()
        case kVK_Escape:
            onCancel?()
        case kVK_Space:
            onTogglePause?()
        default:
            super.keyDown(with: event)
        }
    }
}

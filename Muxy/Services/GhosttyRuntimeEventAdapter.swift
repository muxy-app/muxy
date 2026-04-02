import AppKit
import Foundation
import GhosttyKit

protocol GhosttyRuntimeEventHandling {
    func wakeup()
    func action(app: ghostty_app_t?, target: ghostty_target_s, action: ghostty_action_s) -> Bool
    func readClipboard(userdata: UnsafeMutableRawPointer?, location: ghostty_clipboard_e, state: UnsafeMutableRawPointer?) -> Bool
    func confirmReadClipboard(userdata: UnsafeMutableRawPointer?, content: UnsafePointer<CChar>?, state: UnsafeMutableRawPointer?)
    func writeClipboard(location: ghostty_clipboard_e, content: UnsafePointer<ghostty_clipboard_content_s>?, len: UInt)
    func closeSurface(userdata: UnsafeMutableRawPointer?, needsConfirm: Bool)
}

final class GhosttyRuntimeEventAdapter: GhosttyRuntimeEventHandling {
    func wakeup() {
        DispatchQueue.main.async {
            GhosttyService.shared.tick()
        }
    }

    func action(app: ghostty_app_t?, target: ghostty_target_s, action: ghostty_action_s) -> Bool {
        switch action.tag {
        case GHOSTTY_ACTION_SET_TITLE:
            handleSetTitle(target: target, title: action.action.set_title)
            return true
        case GHOSTTY_ACTION_DESKTOP_NOTIFICATION:
            return true
        default:
            return false
        }
    }

    private func handleSetTitle(target: ghostty_target_s, title: ghostty_action_set_title_s) {
        guard target.tag == GHOSTTY_TARGET_SURFACE else { return }
        guard let surface = target.target.surface else { return }
        guard let userdata = ghostty_surface_userdata(surface) else { return }
        guard let titlePtr = title.title else { return }

        let titleString = String(cString: titlePtr)
        let view = Unmanaged<GhosttyTerminalNSView>.fromOpaque(userdata).takeUnretainedValue()
        DispatchQueue.main.async {
            view.onTitleChange?(titleString)
        }
    }

    func readClipboard(userdata: UnsafeMutableRawPointer?, location: ghostty_clipboard_e, state: UnsafeMutableRawPointer?) -> Bool {
        let text = NSPasteboard.general.string(forType: .string) ?? ""
        text.withCString { ptr in
            ghostty_surface_complete_clipboard_request(
                Self.callbackSurface(from: userdata),
                ptr,
                state,
                false
            )
        }
        return true
    }

    func confirmReadClipboard(userdata: UnsafeMutableRawPointer?, content: UnsafePointer<CChar>?, state: UnsafeMutableRawPointer?) {
        guard let content else { return }
        ghostty_surface_complete_clipboard_request(
            Self.callbackSurface(from: userdata),
            content,
            state,
            true
        )
    }

    func writeClipboard(location: ghostty_clipboard_e, content: UnsafePointer<ghostty_clipboard_content_s>?, len: UInt) {
        guard let content, len > 0 else { return }
        let buffer = UnsafeBufferPointer(start: content, count: Int(len))
        for item in buffer {
            guard let dataPtr = item.data else { continue }
            guard let mimePtr = item.mime else { continue }
            let mime = String(cString: mimePtr)
            guard mime.hasPrefix("text/plain") else { continue }

            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(String(cString: dataPtr), forType: .string)
            return
        }
    }

    func closeSurface(userdata: UnsafeMutableRawPointer?, needsConfirm: Bool) {
        guard let userdata else { return }
        let view = Unmanaged<GhosttyTerminalNSView>.fromOpaque(userdata).takeUnretainedValue()
        DispatchQueue.main.async {
            view.onProcessExit?()
        }
    }

    private static func callbackSurface(from userdata: UnsafeMutableRawPointer?) -> ghostty_surface_t? {
        guard let userdata else { return nil }
        let view = Unmanaged<GhosttyTerminalNSView>.fromOpaque(userdata).takeUnretainedValue()
        return view.surface
    }
}

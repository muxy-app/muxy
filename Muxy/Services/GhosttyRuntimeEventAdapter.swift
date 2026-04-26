import AppKit
import Foundation
import GhosttyKit
import os

private let logger = Logger(subsystem: "app.muxy", category: "RuntimeEventAdapter")

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
            logger.debug("DESKTOP_NOTIFICATION action received")
            handleDesktopNotification(target: target, notification: action.action.desktop_notification)
            return true
        case GHOSTTY_ACTION_START_SEARCH:
            handleStartSearch(target: target, search: action.action.start_search)
            return true
        case GHOSTTY_ACTION_END_SEARCH:
            handleEndSearch(target: target)
            return true
        case GHOSTTY_ACTION_SEARCH_TOTAL:
            handleSearchTotal(target: target, total: action.action.search_total)
            return true
        case GHOSTTY_ACTION_SEARCH_SELECTED:
            handleSearchSelected(target: target, selected: action.action.search_selected)
            return true
        case GHOSTTY_ACTION_COMMAND_FINISHED,
             GHOSTTY_ACTION_SHOW_CHILD_EXITED:
            handleCommandExit(target: target)
            return true
        case GHOSTTY_ACTION_MOUSE_OVER_LINK:
            handleMouseOverLink(target: target, link: action.action.mouse_over_link)
            return true
        case GHOSTTY_ACTION_OPEN_URL:
            return handleOpenURL(target: target, openURL: action.action.open_url)
        default:
            return false
        }
    }

    private func handleSetTitle(target: ghostty_target_s, title: ghostty_action_set_title_s) {
        guard let view = surfaceView(from: target) else { return }
        guard let titlePtr = title.title else { return }
        let titleString = String(cString: titlePtr)
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
            guard !view.processExitHandled else { return }
            view.processExitHandled = true
            view.onProcessExit?()
        }
    }

    private func handleOpenURL(target: ghostty_target_s, openURL: ghostty_action_open_url_s) -> Bool {
        guard let view = surfaceView(from: target) else { return false }
        guard let urlPtr = openURL.url, openURL.len > 0 else { return false }
        let urlString = urlPtr.withMemoryRebound(to: UInt8.self, capacity: Int(openURL.len)) { rawPtr in
            String(bytes: UnsafeBufferPointer(start: rawPtr, count: Int(openURL.len)), encoding: .utf8)
        }
        guard let urlString, let url = URL(string: urlString) else { return false }
        return view.onOpenURL?(url) ?? false
    }

    private func handleMouseOverLink(target: ghostty_target_s, link: ghostty_action_mouse_over_link_s) {
        guard let view = surfaceView(from: target) else { return }
        let hasLink = link.len > 0 && link.url != nil
        DispatchQueue.main.async {
            view.hasOSC8LinkUnderCursor = hasLink
        }
    }

    private func handleCommandExit(target: ghostty_target_s) {
        guard let view = surfaceView(from: target) else { return }
        DispatchQueue.main.async {
            guard view.closesOnCommandExit else { return }
            guard !view.processExitHandled else { return }
            view.processExitHandled = true
            view.onProcessExit?()
        }
    }

    private func handleStartSearch(target: ghostty_target_s, search: ghostty_action_start_search_s) {
        guard let view = surfaceView(from: target) else { return }
        let needle = search.needle.flatMap { String(cString: $0) }
        DispatchQueue.main.async {
            view.onSearchStart?(needle)
        }
    }

    private func handleEndSearch(target: ghostty_target_s) {
        guard let view = surfaceView(from: target) else { return }
        DispatchQueue.main.async {
            view.onSearchEnd?()
        }
    }

    private func handleSearchTotal(target: ghostty_target_s, total: ghostty_action_search_total_s) {
        guard let view = surfaceView(from: target) else { return }
        let value = total.total >= 0 ? Int(total.total) : nil
        DispatchQueue.main.async {
            view.onSearchTotal?(value)
        }
    }

    private func handleSearchSelected(target: ghostty_target_s, selected: ghostty_action_search_selected_s) {
        guard let view = surfaceView(from: target) else { return }
        let value = selected.selected >= 0 ? Int(selected.selected) : nil
        DispatchQueue.main.async {
            view.onSearchSelected?(value)
        }
    }

    private func handleDesktopNotification(
        target: ghostty_target_s,
        notification: ghostty_action_desktop_notification_s
    ) {
        guard let view = surfaceView(from: target) else {
            logger.debug("OSC notification: no surface view from target")
            return
        }
        let rawTitle = notification.title.flatMap { String(cString: $0) } ?? ""
        let title = rawTitle.isEmpty ? "Command executed!" : rawTitle
        let body = notification.body.flatMap { String(cString: $0) } ?? ""
        logger.debug("OSC notification: title=\(title) body=\(body)")
        Task { @MainActor in
            Self.dispatchOSCNotification(view: view, title: title, body: body)
        }
    }

    @MainActor
    private static func dispatchOSCNotification(view: GhosttyTerminalNSView, title: String, body: String) {
        guard let paneID = TerminalViewRegistry.shared.paneID(for: view) else {
            logger.debug("OSC notification: no paneID for view")
            return
        }
        guard let appState = NotificationStore.shared.appState else {
            logger.debug("OSC notification: appState not available")
            return
        }
        logger.debug("OSC notification: dispatching to store, paneID=\(paneID)")
        NotificationStore.shared.add(
            paneID: paneID,
            source: .osc,
            title: title,
            body: body,
            appState: appState
        )
    }

    private func surfaceView(from target: ghostty_target_s) -> GhosttyTerminalNSView? {
        guard target.tag == GHOSTTY_TARGET_SURFACE else { return nil }
        guard let surface = target.target.surface else { return nil }
        guard let userdata = ghostty_surface_userdata(surface) else { return nil }
        return Unmanaged<GhosttyTerminalNSView>.fromOpaque(userdata).takeUnretainedValue()
    }

    private static func callbackSurface(from userdata: UnsafeMutableRawPointer?) -> ghostty_surface_t? {
        guard let userdata else { return nil }
        let view = Unmanaged<GhosttyTerminalNSView>.fromOpaque(userdata).takeUnretainedValue()
        return view.surface
    }
}

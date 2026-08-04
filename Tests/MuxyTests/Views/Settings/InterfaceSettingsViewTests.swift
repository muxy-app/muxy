import AppKit
import SwiftUI
import Testing

@testable import Muxy

@MainActor
@Suite("Interface settings view")
struct InterfaceSettingsViewTests {
    @Test("App Layout picker fits minimum settings content width")
    func appLayoutPickerFitsMinimumSettingsContentWidth() throws {
        let contentSize = NSSize(width: SettingsMetrics.minimumContentWidth, height: 40)
        let hostingView = NSHostingView(
            rootView: SettingsRow("App Layout") {
                AppLayoutPicker(selection: .constant(.projectFocused))
            }
                .frame(width: contentSize.width, height: contentSize.height)
        )
        hostingView.frame = NSRect(origin: .zero, size: contentSize)
        hostingView.layoutSubtreeIfNeeded()

        let picker = try #require(appLayoutPicker(in: hostingView))
        let pickerFrame = picker.convert(picker.bounds, to: hostingView)

        #expect(pickerFrame.minX >= hostingView.bounds.minX)
        #expect(pickerFrame.maxX <= hostingView.bounds.maxX)
        #expect(picker.bounds.width >= picker.intrinsicContentSize.width)
    }

    private func appLayoutPicker(in view: NSView) -> NSSegmentedControl? {
        if let control = view as? NSSegmentedControl,
           control.segmentCount == AppLayout.allCases.count,
           control.label(forSegment: control.segmentCount - 1) == L10n.string(key: AppLayout.agentsFocused.title)
        {
            return control
        }
        for subview in view.subviews {
            if let control = appLayoutPicker(in: subview) {
                return control
            }
        }
        return nil
    }
}

import AppKit
import CodeEditLanguages
import CodeEditSourceEditor
import SwiftUI

struct CodeEditorView: View {
    @Bindable var state: EditorTabState
    @Environment(GhosttyService.self) private var ghostty

    var body: some View {
        SourceEditor(
            $state.content,
            language: CodeLanguage.detectLanguageFrom(
                url: URL(fileURLWithPath: state.filePath)
            ),
            configuration: editorConfiguration,
            state: $state.editorState
        )
        .onChange(of: state.content) {
            state.markModified()
        }
    }

    private var editorConfiguration: SourceEditorConfiguration {
        let fg = GhosttyService.shared.foregroundColor
        let bg = GhosttyService.shared.backgroundColor

        let theme = EditorTheme(
            text: .init(color: fg),
            insertionPoint: fg,
            invisibles: .init(color: fg.withAlphaComponent(0.2)),
            background: bg,
            lineHighlight: fg.withAlphaComponent(0.06),
            selection: fg.withAlphaComponent(0.15),
            keywords: .init(
                color: GhosttyService.shared.paletteColor(at: 4) ?? .systemBlue,
                bold: true
            ),
            commands: .init(
                color: GhosttyService.shared.paletteColor(at: 6) ?? .systemCyan
            ),
            types: .init(
                color: GhosttyService.shared.paletteColor(at: 5) ?? .systemPurple
            ),
            attributes: .init(
                color: GhosttyService.shared.paletteColor(at: 6) ?? .systemCyan
            ),
            variables: .init(color: fg),
            values: .init(
                color: GhosttyService.shared.paletteColor(at: 3) ?? .systemYellow
            ),
            numbers: .init(
                color: GhosttyService.shared.paletteColor(at: 3) ?? .systemYellow
            ),
            strings: .init(
                color: GhosttyService.shared.paletteColor(at: 2) ?? .systemGreen
            ),
            characters: .init(
                color: GhosttyService.shared.paletteColor(at: 1) ?? .systemOrange
            ),
            comments: .init(
                color: GhosttyService.shared.paletteColor(at: 8) ?? .systemGray,
                italic: true
            )
        )

        return SourceEditorConfiguration(
            appearance: .init(
                theme: theme,
                font: NSFont.monospacedSystemFont(ofSize: 13, weight: .regular),
                wrapLines: true
            ),
            behavior: .init(indentOption: .spaces(count: 4)),
            peripherals: .init(showMinimap: false)
        )
    }
}

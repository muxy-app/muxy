import Foundation
import MuxyShared
import Testing
@testable import Muxy

@Suite("Client theme")
@MainActor
struct ClientThemeTests {
    private func makeTheme(
        cursorColor: UInt32? = nil,
        cursorText: UInt32? = nil,
        selectionBackground: UInt32? = nil,
        selectionForeground: UInt32? = nil
    ) -> ClientThemeDTO {
        ClientThemeDTO(
            fg: 0xD4D4D4,
            bg: 0x141414,
            palette: (0 ..< 16).map { UInt32($0) },
            cursorColor: cursorColor,
            cursorText: cursorText,
            selectionBackground: selectionBackground,
            selectionForeground: selectionForeground
        )
    }

    @Test("config text serializes the full color set in ghostty format")
    func configTextFullSet() {
        let theme = makeTheme(
            cursorColor: 0xD4D4D4,
            cursorText: 0x141414,
            selectionBackground: 0x2C2C2C,
            selectionForeground: 0xE4E4E4
        )
        let text = ClientThemeApplier.configText(for: theme)

        #expect(text.contains("palette = 0=#000000"))
        #expect(text.contains("palette = 15=#00000f"))
        #expect(text.contains("background = #141414"))
        #expect(text.contains("foreground = #d4d4d4"))
        #expect(text.contains("cursor-color = #d4d4d4"))
        #expect(text.contains("cursor-text = #141414"))
        #expect(text.contains("selection-background = #2c2c2c"))
        #expect(text.contains("selection-foreground = #e4e4e4"))
    }

    @Test("config text omits unset optional colors")
    func configTextOmitsOptionalColors() {
        let text = ClientThemeApplier.configText(for: makeTheme())

        #expect(text.contains("background = #141414"))
        #expect(text.contains("foreground = #d4d4d4"))
        #expect(!text.contains("cursor-color"))
        #expect(!text.contains("cursor-text"))
        #expect(!text.contains("selection-background"))
        #expect(!text.contains("selection-foreground"))
    }

    @Test("config text caps palette at 16 entries")
    func configTextCapsPalette() {
        let theme = ClientThemeDTO(fg: 1, bg: 2, palette: (0 ..< 32).map { UInt32($0) })
        let text = ClientThemeApplier.configText(for: theme)

        #expect(text.contains("palette = 15="))
        #expect(!text.contains("palette = 16="))
    }

    @Test("client theme DTO round-trips with and without optionals")
    func dtoRoundTrip() throws {
        for theme in [makeTheme(), makeTheme(cursorColor: 0xFFFFFF, selectionBackground: 0x101010)] {
            let data = try JSONEncoder().encode(theme)
            let decoded = try JSONDecoder().decode(ClientThemeDTO.self, from: data)
            #expect(decoded == theme)
        }
    }

    @Test("set client theme params encode a nil theme as a clear")
    func clearParamsRoundTrip() throws {
        let params = SetClientThemeParams(theme: nil)
        let data = try JSONEncoder().encode(params)
        let decoded = try JSONDecoder().decode(SetClientThemeParams.self, from: data)
        #expect(decoded.theme == nil)
    }

    @Test("client-owned pane resolves to the client theme, mac-owned resolves to none")
    func ownershipResolvesActiveTheme() {
        let ownership = PaneOwnershipStore.shared
        let store = ClientThemeStore.shared
        let clientID = UUID()
        let ownedPane = UUID()
        let macPane = UUID()
        let theme = makeTheme()
        defer {
            ownership.releaseAll(clientID: clientID)
            ownership.releaseToMac(paneID: macPane)
            store.clear(for: clientID)
        }

        store.setTheme(theme, for: clientID)
        ownership.assign(paneID: ownedPane, to: clientID)

        #expect(resolveActiveTheme(ownership: ownership, store: store, paneID: ownedPane) == theme)
        #expect(resolveActiveTheme(ownership: ownership, store: store, paneID: macPane) == nil)

        ownership.releaseToMac(paneID: ownedPane)
        #expect(resolveActiveTheme(ownership: ownership, store: store, paneID: ownedPane) == nil)
    }

    @Test("store caps an oversized palette so retained themes stay bounded")
    func storeCapsOversizedPalette() {
        let store = ClientThemeStore.shared
        let clientID = UUID()
        defer { store.clear(for: clientID) }

        store.setTheme(ClientThemeDTO(fg: 1, bg: 2, palette: (0 ..< 64).map { UInt32($0) }), for: clientID)

        #expect(store.theme(for: clientID)?.palette.count == ClientThemeDTO.paletteLimit)
    }

    @Test("capped is a no-op for an in-range palette")
    func cappedKeepsInRangePalette() {
        let theme = makeTheme()
        #expect(theme.capped() == theme)
    }

    private func resolveActiveTheme(
        ownership: PaneOwnershipStore,
        store: ClientThemeStore,
        paneID: UUID
    ) -> ClientThemeDTO? {
        guard let clientID = ownership.remoteOwner(for: paneID) else { return nil }
        return store.theme(for: clientID)
    }
}

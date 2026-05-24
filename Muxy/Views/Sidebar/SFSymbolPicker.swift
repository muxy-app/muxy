import SwiftUI

struct SFSymbolPicker: View {
    var title: String = "Icon"
    let selectedName: String?
    let onSelect: (String?) -> Void

    @State private var searchText = ""

    private static let symbols: [(name: String, systemName: String)] = [
        ("Folder", "folder"),
        ("Folder Fill", "folder.fill"),
        ("Terminal", "terminal"),
        ("Terminal Fill", "terminal.fill"),
        ("Code", "chevron.left.forwardslash.chevron.right"),
        ("Document", "doc"),
        ("Document Fill", "doc.fill"),
        ("Document Text", "doc.text"),
        ("Document Text Fill", "doc.text.fill"),
        ("Document on Clipboard", "doc.on.clipboard"),
        ("List", "list.bullet"),
        ("List Rectangle", "list.bullet.rectangle"),
        ("Pencil", "pencil"),
        ("Pencil and Ruler", "pencil.and.ruler"),
        ("Ruler", "ruler"),
        ("Swatchpalette", "swatchpalette.fill"),
        ("Paintpalette", "paintpalette.fill"),
        ("Sparkle", "sparkle"),
        ("Star", "star"),
        ("Star Fill", "star.fill"),
        ("Gear", "gearshape"),
        ("Gear Fill", "gearshape.fill"),
        ("Wrench", "wrench"),
        ("Wrench Fill", "wrench.fill"),
        ("Hammer", "hammer"),
        ("Hammer Fill", "hammer.fill"),
        ("Screwdriver", "screwdriver"),
        ("Screwdriver Fill", "screwdriver.fill"),
        ("Wand and Stars", "wand.and.stars"),
        ("Globe", "globe"),
        ("Globe Americas", "globe.americas"),
        ("Network", "network"),
        ("Server Rack", "server.rack"),
        ("External Drive", "externaldrive"),
        ("External Drive Fill", "externaldrive.fill"),
        ("Optical Disc", "opticaldisc"),
        ("Optical Disc Fill", "opticaldisc.fill"),
        ("Cloud", "cloud"),
        ("Cloud Fill", "cloud.fill"),
        ("Square Grid 2x2", "square.grid.2x2"),
        ("Square Grid 2x2 Fill", "square.grid.2x2.fill"),
        ("Square Grid 3x2", "square.grid.3x2"),
        ("Square Grid 3x2 Fill", "square.grid.3x2.fill"),
        ("Rectangle Grid 1x2", "rectangle.grid.1x2"),
        ("Rectangle Grid 1x2 Fill", "rectangle.grid.1x2.fill"),
        ("Rectangle 3 Group", "rectangle.3.group"),
        ("Rectangle 3 Group Fill", "rectangle.3.group.fill"),
        ("Square Stack", "square.stack"),
        ("Square Stack Fill", "square.stack.fill"),
        ("Circle", "circle"),
        ("Circle Fill", "circle.fill"),
        ("Diamond", "diamond"),
        ("Diamond Fill", "diamond.fill"),
        ("Hexagon", "hexagon"),
        ("Hexagon Fill", "hexagon.fill"),
        ("App", "app"),
        ("App Fill", "app.fill"),
        ("App Badge", "app.badge"),
        ("App Badge Fill", "app.badge.fill"),
        ("App Window", "macwindow"),
        ("Play", "play"),
        ("Play Fill", "play.fill"),
        ("Play Rectangle", "play.rectangle"),
        ("Play Rectangle Fill", "play.rectangle.fill"),
        ("Music Note", "music.note"),
        ("Music Note List", "music.note.list"),
        ("Film", "film"),
        ("Film Fill", "film.fill"),
        ("Photo", "photo"),
        ("Photo Fill", "photo.fill"),
        ("Video", "video"),
        ("Video Fill", "video.fill"),
        ("Message", "message"),
        ("Message Fill", "message.fill"),
        ("Bubble Left", "bubble.left"),
        ("Bubble Left Fill", "bubble.left.fill"),
        ("Envelope", "envelope"),
        ("Envelope Fill", "envelope.fill"),
        ("Calendar", "calendar"),
        ("Clock", "clock"),
        ("Clock Fill", "clock.fill"),
        ("Timer", "timer"),
        ("Bell", "bell"),
        ("Bell Fill", "bell.fill"),
        ("Bell Badge", "bell.badge"),
        ("Bell Badge Fill", "bell.badge.fill"),
        ("Bookmark", "bookmark"),
        ("Bookmark Fill", "bookmark.fill"),
        ("Flag", "flag"),
        ("Flag Fill", "flag.fill"),
        ("Tag", "tag"),
        ("Tag Fill", "tag.fill"),
        ("Book", "book"),
        ("Book Fill", "book.fill"),
        ("Books Vertical", "books.vertical"),
        ("Books Vertical Fill", "books.vertical.fill"),
        ("Magnifying Glass", "magnifyingglass"),
        ("Lightbulb", "lightbulb"),
        ("Lightbulb Fill", "lightbulb.fill"),
        ("Cube", "cube"),
        ("Cube Fill", "cube.fill"),
        ("Cylinder", "cylinder"),
        ("Cylinder Fill", "cylinder.fill"),
        ("Capsule", "capsule"),
        ("Capsule Fill", "capsule.fill"),
        ("Pyramid", "pyramid"),
        ("Pyramid Fill", "pyramid.fill"),
        ("Cone", "cone"),
        ("Cone Fill", "cone.fill"),
        ("Key", "key"),
        ("Key Fill", "key.fill"),
        ("Lock", "lock"),
        ("Lock Fill", "lock.fill"),
        ("Lock Shield", "lock.shield"),
        ("Lock Shield Fill", "lock.shield.fill"),
        ("Shield", "shield"),
        ("Shield Fill", "shield.fill"),
        ("Bolt", "bolt"),
        ("Bolt Fill", "bolt.fill"),
        ("Bolt Shield", "bolt.shield"),
        ("Bolt Shield Fill", "bolt.shield.fill"),
        ("Waveform", "waveform"),
        ("Waveform Path", "waveform.path"),
        ("Chart Pie", "chart.pie"),
        ("Chart Pie Fill", "chart.pie.fill"),
        ("Chart Bar", "chart.bar"),
        ("Chart Bar Fill", "chart.bar.fill"),
        ("Chart Line", "chart.line.uptrend.xyaxis"),
        ("Gauge", "gauge"),
        ("Speedometer", "speedometer"),
        ("Viewfinder", "viewfinder"),
        ("Viewfinder Circle", "viewfinder.circle"),
        ("Camera", "camera"),
        ("Camera Fill", "camera.fill"),
        ("Eye", "eye"),
        ("Eye Fill", "eye.fill"),
        ("Eye Slash", "eye.slash"),
        ("Eye Slash Fill", "eye.slash.fill"),
        ("Person", "person"),
        ("Person Fill", "person.fill"),
        ("Person Circle", "person.circle"),
        ("Person Circle Fill", "person.circle.fill"),
        ("Person 2", "person.2"),
        ("Person 2 Fill", "person.2.fill"),
        ("Person 3", "person.3"),
        ("Person 3 Fill", "person.3.fill"),
        ("Backpack", "backpack"),
        ("Backpack Fill", "backpack.fill"),
        ("House", "house"),
        ("House Fill", "house.fill"),
        ("Building", "building"),
        ("Building Fill", "building.fill"),
        ("Building Columns", "building.columns"),
        ("Building Columns Fill", "building.columns.fill"),
        ("Cart", "cart"),
        ("Cart Fill", "cart.fill"),
        ("Bag", "bag"),
        ("Bag Fill", "bag.fill"),
        ("Creditcard", "creditcard"),
        ("Creditcard Fill", "creditcard.fill"),
        ("Link", "link"),
        ("Link Circle", "link.circle"),
        ("Link Circle Fill", "link.circle.fill"),
        ("Square and Arrow", "square.and.arrow.up"),
        ("Square and Arrow Fill", "square.and.arrow.up.fill"),
        ("Square on Square", "square.on.square"),
        ("Square on Square Fill", "square.on.square.fill"),
        ("Arrowshape", "arrowshape.turn.up.forward"),
        ("Arrowshape Fill", "arrowshape.turn.up.forward.fill"),
        ("Arrow Up", "arrow.up"),
        ("Arrow Down", "arrow.down"),
        ("Arrow Left", "arrow.left"),
        ("Arrow Right", "arrow.right"),
        ("Arrow Up Circle", "arrow.up.circle"),
        ("Arrow Up Circle Fill", "arrow.up.circle.fill"),
        ("Arrow Down Circle", "arrow.down.circle"),
        ("Arrow Down Circle Fill", "arrow.down.circle.fill"),
        ("Arrow Left Circle", "arrow.left.circle"),
        ("Arrow Left Circle Fill", "arrow.left.circle.fill"),
        ("Arrow Right Circle", "arrow.right.circle"),
        ("Arrow Right Circle Fill", "arrow.right.circle.fill"),
        ("Arrow Left Right", "arrow.left.and.right"),
        ("Arrow Up Down", "arrow.up.and.down"),
        ("Arrows", "arrow.triangle.branch"),
        ("Arrows Merge", "arrow.triangle.merge"),
        ("Arrows Swap", "arrow.triangle.swap"),
        ("Arrowshape Backward", "arrowshape.backward"),
        ("Arrowshape Forward", "arrowshape.forward"),
        ("Go Backward", "gobackward"),
        ("Go Forward", "goforward"),
        ("Forward", "forward"),
        ("Forward Fill", "forward.fill"),
    ]

    private var filteredSymbols: [(name: String, systemName: String)] {
        guard !searchText.isEmpty else { return Self.symbols }
        return Self.symbols.filter {
            $0.name.localizedCaseInsensitiveContains(searchText)
                || $0.systemName.localizedCaseInsensitiveContains(searchText)
        }
    }

    private let columns = Array(
        repeating: GridItem(.fixed(UIMetrics.controlLarge), spacing: UIMetrics.spacing4),
        count: 7
    )

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing5) {
            Text(title)
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)

            TextField("Search symbols...", text: $searchText)
                .textFieldStyle(.roundedBorder)
                .font(.system(size: UIMetrics.fontFootnote))

            ScrollView {
                LazyVGrid(columns: columns, spacing: UIMetrics.spacing4) {
                    ForEach(filteredSymbols, id: \.systemName) { symbol in
                        symbolButton(symbol)
                    }
                }
            }
            .frame(height: UIMetrics.scaled(260))

            Divider()

            Button {
                onSelect(nil)
            } label: {
                HStack(spacing: UIMetrics.spacing3) {
                    Image(systemName: "xmark.circle")
                        .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                    Text("Remove Icon")
                        .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                }
                .foregroundStyle(MuxyTheme.fgMuted)
            }
            .buttonStyle(.plain)
            .disabled(selectedName == nil)
            .opacity(selectedName == nil ? 0.4 : 1)
        }
        .padding(UIMetrics.spacing6)
        .frame(width: UIMetrics.scaled(300))
    }

    private func symbolButton(_ symbol: (name: String, systemName: String)) -> some View {
        let isSelected = selectedName == symbol.systemName
        return Button {
            onSelect(symbol.systemName)
        } label: {
            ZStack {
                RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                    .fill(isSelected ? MuxyTheme.accent.opacity(0.2) : Color.clear)
                    .frame(width: UIMetrics.controlLarge, height: UIMetrics.controlLarge)

                Image(systemName: symbol.systemName)
                    .font(.system(size: UIMetrics.fontTitleLarge, weight: .regular))
                    .foregroundStyle(isSelected ? MuxyTheme.accent : MuxyTheme.fg)
            }
            .frame(width: UIMetrics.controlLarge, height: UIMetrics.controlLarge)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(symbol.name)
        .accessibilityLabel(symbol.name)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }
}

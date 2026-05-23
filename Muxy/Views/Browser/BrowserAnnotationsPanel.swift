import SwiftUI

struct BrowserAnnotationsPanel: View {
    @Bindable var session: BrowserSession

    var body: some View {
        VStack(spacing: 0) {
            header
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            if session.inspector.annotations.isEmpty {
                emptyState
            } else {
                list
            }
        }
        .background(MuxyTheme.bg)
    }

    private var header: some View {
        HStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "square.dashed")
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text("Elements")
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Spacer()
            Button {
                session.inspector.showsAnnotationsPanel = false
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: UIMetrics.controlSmall, height: UIMetrics.controlSmall)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Hide elements panel")
        }
        .padding(.horizontal, UIMetrics.spacing4)
        .frame(height: UIMetrics.scaled(28))
    }

    private var emptyState: some View {
        VStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "cursorarrow.click")
                .font(.system(size: UIMetrics.fontTitle))
                .foregroundStyle(MuxyTheme.fgDim)
            Text(
                "Click an element to capture it. "
                    + "Add a quick note: what looks wrong, what should it look like, "
                    + "and any reference file or design."
            )
            .font(.system(size: UIMetrics.fontFootnote))
            .multilineTextAlignment(.center)
            .foregroundStyle(MuxyTheme.fgMuted)
            .padding(.horizontal, UIMetrics.spacing4)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var list: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                ForEach(session.inspector.annotations) { annotation in
                    AnnotationRow(session: session, annotationID: annotation.id)
                }
            }
            .padding(UIMetrics.spacing4)
        }
    }
}

private struct AnnotationRow: View {
    @Bindable var session: BrowserSession
    let annotationID: UUID
    @Environment(AppState.self) private var appState
    @State private var draftComment: String = ""
    @State private var showsStylePopover = false

    private var annotation: BrowserAnnotation? {
        session.inspector.annotations.first(where: { $0.id == annotationID })
    }

    var body: some View {
        if let annotation {
            VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
                HStack(spacing: UIMetrics.spacing2) {
                    statusIndicator(for: annotation)
                    Text(annotation.textSnippet.isEmpty ? annotation.selector : annotation.textSnippet)
                        .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fg)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Spacer()
                    Button {
                        showsStylePopover = true
                    } label: {
                        Image(systemName: "paintbrush")
                            .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                            .foregroundStyle(MuxyTheme.fgMuted)
                    }
                    .buttonStyle(.plain)
                    .popover(isPresented: $showsStylePopover, arrowEdge: .leading) {
                        StyleInspectorPopover(session: session, annotationID: annotationID)
                    }
                    .help("Edit styles")
                    .accessibilityLabel("Edit styles")

                    Button {
                        session.inspector.removeAnnotation(id: annotationID)
                    } label: {
                        Image(systemName: "trash")
                            .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                            .foregroundStyle(MuxyTheme.fgMuted)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Delete annotation")
                }

                Text(annotation.selector)
                    .font(.system(size: UIMetrics.fontXS, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .lineLimit(1)
                    .truncationMode(.middle)

                if let screenshotURL = annotation.screenshotURL {
                    ScreenshotThumbnail(url: screenshotURL)
                }

                if !annotation.styleOverrides.isEmpty {
                    VStack(alignment: .leading, spacing: 2) {
                        ForEach(annotation.styleOverrides) { override in
                            Text("• \(override.property.displayName): \(override.value)")
                                .font(.system(size: UIMetrics.fontXS, design: .monospaced))
                                .foregroundStyle(MuxyTheme.fgMuted)
                        }
                    }
                }

                commentEditor(annotation: annotation)

                HStack(spacing: UIMetrics.spacing2) {
                    Spacer()
                    Button("Send to Terminal") {
                        BrowserAnnotationSender.send(
                            annotation: annotation,
                            from: session,
                            appState: appState,
                            markSent: { session.inspector.markAnnotationSent(annotationID) }
                        )
                    }
                    .buttonStyle(.plain)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    .foregroundStyle(.white)
                    .padding(.horizontal, UIMetrics.spacing4)
                    .padding(.vertical, UIMetrics.spacing2)
                    .background(MuxyTheme.accent, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                    .help("Insert markdown into the focused terminal pane")
                }
            }
            .padding(UIMetrics.spacing3)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .overlay(
                RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                    .strokeBorder(MuxyTheme.border, lineWidth: 1)
            )
            .onAppear { draftComment = annotation.comment }
        }
    }

    private func statusIndicator(for annotation: BrowserAnnotation) -> some View {
        Circle()
            .fill(annotation.status == .sent ? MuxyTheme.accent : MuxyTheme.warning)
            .frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
            .accessibilityLabel(annotation.status == .sent ? "Sent" : "Draft")
    }

    private func commentEditor(annotation: BrowserAnnotation) -> some View {
        let binding = Binding(
            get: { draftComment.isEmpty && !annotation.comment.isEmpty ? annotation.comment : draftComment },
            set: { newValue in
                draftComment = newValue
                session.inspector.updateComment(annotationID: annotationID, comment: newValue)
            }
        )
        let showsPlaceholder = binding.wrappedValue.isEmpty
        return ZStack(alignment: .topLeading) {
            TextEditor(text: binding)
                .font(.system(size: UIMetrics.fontFootnote))
                .frame(minHeight: UIMetrics.scaled(56))
                .padding(UIMetrics.spacing2)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                .overlay(
                    RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                        .strokeBorder(MuxyTheme.border, lineWidth: 1)
                )
                .scrollContentBackground(.hidden)
            if showsPlaceholder {
                Text("What's wrong? What should it look like? Reference file or design?")
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .padding(.horizontal, UIMetrics.spacing3)
                    .padding(.vertical, UIMetrics.spacing3)
                    .allowsHitTesting(false)
            }
        }
    }
}

private struct ScreenshotThumbnail: View {
    let url: URL

    var body: some View {
        Button {
            NSWorkspace.shared.open(url)
        } label: {
            thumbnailContent
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .buttonStyle(.plain)
        .help("Open screenshot in Preview")
        .accessibilityLabel("Open screenshot in Preview")
    }

    @ViewBuilder
    private var thumbnailContent: some View {
        if let image = NSImage(contentsOf: url) {
            Image(nsImage: image)
                .resizable()
                .interpolation(.high)
                .scaledToFit()
                .frame(maxHeight: UIMetrics.scaled(64))
                .cornerRadius(UIMetrics.radiusSM)
                .overlay(
                    RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                        .strokeBorder(MuxyTheme.border, lineWidth: 1)
                )
        } else {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: "photo")
                    .foregroundStyle(MuxyTheme.fgDim)
                Text("Screenshot pending")
                    .font(.system(size: UIMetrics.fontXS))
                    .foregroundStyle(MuxyTheme.fgDim)
            }
            .frame(height: UIMetrics.scaled(64))
        }
    }
}

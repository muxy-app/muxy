import SwiftUI

struct BrowserAnnotationsPanel: View {
    @Bindable var state: BrowserTabState

    var body: some View {
        VStack(spacing: 0) {
            header
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            if state.annotations.isEmpty {
                emptyState
            } else {
                list
            }
        }
        .background(MuxyTheme.bg)
    }

    private var header: some View {
        HStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "bubble.left.and.bubble.right")
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text("Annotations")
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Spacer()
            Button {
                state.showsAnnotationsPanel = false
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: UIMetrics.controlSmall, height: UIMetrics.controlSmall)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Hide annotations panel")
        }
        .padding(.horizontal, UIMetrics.spacing4)
        .frame(height: UIMetrics.scaled(28))
    }

    private var emptyState: some View {
        VStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "cursorarrow.click")
                .font(.system(size: UIMetrics.fontTitle))
                .foregroundStyle(MuxyTheme.fgDim)
            Text("Toggle annotate mode and click an element to leave feedback.")
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
                ForEach(state.annotations) { annotation in
                    AnnotationRow(state: state, annotationID: annotation.id)
                }
            }
            .padding(UIMetrics.spacing4)
        }
    }
}

private struct AnnotationRow: View {
    @Bindable var state: BrowserTabState
    let annotationID: UUID
    @Environment(AppState.self) private var appState
    @State private var draftComment: String = ""
    @State private var showsStylePopover = false

    private var annotation: BrowserAnnotation? {
        state.annotations.first(where: { $0.id == annotationID })
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
                        Image(systemName: "slider.horizontal.3")
                            .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                            .foregroundStyle(MuxyTheme.fgMuted)
                    }
                    .buttonStyle(.plain)
                    .popover(isPresented: $showsStylePopover, arrowEdge: .leading) {
                        StyleInspectorPopover(state: state, annotationID: annotationID)
                    }
                    .help("Style controls")

                    Button {
                        state.removeAnnotation(id: annotationID)
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

                if !annotation.styleOverrides.isEmpty {
                    VStack(alignment: .leading, spacing: 2) {
                        ForEach(annotation.styleOverrides) { override in
                            Text("• \(override.property.displayName): \(override.value)")
                                .font(.system(size: UIMetrics.fontXS, design: .monospaced))
                                .foregroundStyle(MuxyTheme.fgMuted)
                        }
                    }
                }

                TextEditor(text: Binding(
                    get: { draftComment.isEmpty && !annotation.comment.isEmpty ? annotation.comment : draftComment },
                    set: { newValue in
                        draftComment = newValue
                        state.updateComment(annotationID: annotationID, comment: newValue)
                    }
                ))
                .font(.system(size: UIMetrics.fontFootnote))
                .frame(minHeight: UIMetrics.scaled(56))
                .padding(UIMetrics.spacing2)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                .overlay(
                    RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                        .strokeBorder(MuxyTheme.border, lineWidth: 1)
                )
                .scrollContentBackground(.hidden)

                HStack(spacing: UIMetrics.spacing2) {
                    Spacer()
                    Button("Send to Terminal") {
                        BrowserAnnotationSender.send(
                            annotation: annotation,
                            from: state,
                            appState: appState,
                            markSent: { state.markAnnotationSent(annotationID) }
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
}

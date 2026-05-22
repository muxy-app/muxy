import SwiftUI

struct StyleInspectorPopover: View {
    @Bindable var session: BrowserSession
    let annotationID: UUID

    private var annotation: BrowserAnnotation? {
        session.inspector.annotations.first(where: { $0.id == annotationID })
    }

    private var computedSeed: [String: String] {
        session.inspector.computedStyleSeeds[annotationID] ?? [:]
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
            Text("Style Overrides")
                .font(.system(size: UIMetrics.fontEmphasis, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Text(annotation?.selector ?? "")
                .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgDim)
                .lineLimit(1)
                .truncationMode(.middle)

            ScrollView {
                VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                    ForEach(StyleOverride.Property.allCases) { property in
                        StylePropertyRow(
                            session: session,
                            annotationID: annotationID,
                            property: property,
                            originalValue: computedSeed[property.cssName] ?? ""
                        )
                    }
                }
                .padding(.vertical, UIMetrics.spacing2)
            }
            .frame(width: UIMetrics.scaled(280), height: UIMetrics.scaled(320))
        }
        .padding(UIMetrics.spacing4)
    }
}

private struct StylePropertyRow: View {
    @Bindable var session: BrowserSession
    let annotationID: UUID
    let property: StyleOverride.Property
    let originalValue: String

    @State private var draftValue: String = ""

    private var existingOverride: StyleOverride? {
        session.inspector.annotations
            .first(where: { $0.id == annotationID })?
            .styleOverrides
            .first(where: { $0.property == property })
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: UIMetrics.spacing2) {
                Text(property.displayName)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
                Spacer()
                if existingOverride != nil {
                    Button {
                        if let override = existingOverride {
                            session.removeStyleOverride(id: override.id, for: annotationID)
                            draftValue = ""
                        }
                    } label: {
                        Image(systemName: "arrow.uturn.backward.circle")
                            .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                            .foregroundStyle(MuxyTheme.fgMuted)
                    }
                    .buttonStyle(.plain)
                    .help("Reset")
                }
            }
            HStack(spacing: UIMetrics.spacing2) {
                Text(originalValue.isEmpty ? "—" : originalValue)
                    .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .lineLimit(1)
                    .truncationMode(.tail)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Image(systemName: "arrow.right")
                    .font(.system(size: UIMetrics.fontCaption))
                    .foregroundStyle(MuxyTheme.fgDim)
                TextField("override", text: $draftValue)
                    .textFieldStyle(.plain)
                    .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                    .padding(.horizontal, UIMetrics.spacing2)
                    .frame(height: UIMetrics.scaled(20))
                    .frame(maxWidth: .infinity)
                    .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                    .overlay(
                        RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                            .strokeBorder(MuxyTheme.border, lineWidth: 1)
                    )
                    .onSubmit(commit)
            }
        }
        .onAppear {
            draftValue = existingOverride?.value ?? ""
        }
    }

    private func commit() {
        guard let annotation = session.inspector.annotations.first(where: { $0.id == annotationID }) else { return }
        let trimmed = draftValue.trimmingCharacters(in: .whitespacesAndNewlines)
        let sanitized = BrowserAnnotationSanitizer.sanitizeStyleValue(trimmed)
        if sanitized.isEmpty {
            if let override = existingOverride {
                session.removeStyleOverride(id: override.id, for: annotationID)
            }
            draftValue = ""
            return
        }
        draftValue = sanitized
        let override = StyleOverride(
            id: existingOverride?.id ?? UUID(),
            selector: annotation.selector,
            property: property,
            originalValue: originalValue,
            value: sanitized
        )
        session.upsertStyleOverride(override, for: annotationID)
    }
}

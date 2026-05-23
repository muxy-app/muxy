import SwiftUI

struct StyleInspectorPopover: View {
    @Bindable var session: BrowserSession
    let annotationID: UUID

    private var annotation: BrowserAnnotation? {
        session.inspector.annotations.first(where: { $0.id == annotationID })
    }

    private var computedSeed: [String: String] {
        annotation?.computedStyle ?? [:]
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
            header
            Rectangle()
                .fill(MuxyTheme.border)
                .frame(height: 1)
            ScrollView {
                VStack(alignment: .leading, spacing: UIMetrics.spacing5) {
                    typographySection
                    colorSection
                    boxModelSection(
                        title: "Padding",
                        top: .paddingTop,
                        right: .paddingRight,
                        bottom: .paddingBottom,
                        left: .paddingLeft
                    )
                    boxModelSection(
                        title: "Margin",
                        top: .marginTop,
                        right: .marginRight,
                        bottom: .marginBottom,
                        left: .marginLeft
                    )
                    borderSection
                }
                .padding(.vertical, UIMetrics.spacing2)
            }
            .frame(height: UIMetrics.scaled(340))
        }
        .frame(width: UIMetrics.scaled(232))
        .padding(UIMetrics.spacing4)
    }

    private var header: some View {
        let fullSelector = annotation?.selector ?? ""
        let segments = fullSelector.components(separatedBy: " > ")
        let leaf = segments.last ?? ""
        let ancestorPath = segments.dropLast().joined(separator: " > ")
        return VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            HStack(spacing: UIMetrics.spacing2) {
                Text("Style Overrides")
                    .font(.system(size: UIMetrics.fontEmphasis, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
                Spacer(minLength: 0)
                copySelectorButton(selector: fullSelector)
            }
            selectorDisplay(leaf: leaf, ancestorPath: ancestorPath, fullSelector: fullSelector)
        }
    }

    private func selectorDisplay(
        leaf: String,
        ancestorPath: String,
        fullSelector: String
    ) -> some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
            Text(leaf.isEmpty ? "—" : leaf)
                .font(.system(size: UIMetrics.fontCaption, weight: .medium, design: .monospaced))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(2)
                .truncationMode(.tail)
                .frame(maxWidth: .infinity, alignment: .leading)
                .textSelection(.enabled)
                .help(fullSelector)
            if !ancestorPath.isEmpty {
                Text(ancestorPath)
                    .font(.system(size: UIMetrics.fontXS, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .lineLimit(1)
                    .truncationMode(.head)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .textSelection(.enabled)
                    .help(fullSelector)
            }
        }
    }

    @ViewBuilder
    private func copySelectorButton(selector: String) -> some View {
        if !selector.isEmpty {
            Button {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(selector, forType: .string)
            } label: {
                Image(systemName: "doc.on.doc")
                    .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: UIMetrics.scaled(18), height: UIMetrics.scaled(18))
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Copy selector")
            .accessibilityLabel("Copy selector")
        }
    }

    private var typographySection: some View {
        section(title: "Typography") {
            propertyRow(.fontFamily, label: "Family")
            propertyRow(.fontSize, label: "Size")
            propertyRow(.fontWeight, label: "Weight")
        }
    }

    private var colorSection: some View {
        section(title: "Color") {
            propertyRow(.color, label: "Text")
            propertyRow(.backgroundColor, label: "Background")
        }
    }

    private var borderSection: some View {
        section(title: "Border") {
            propertyRow(.borderRadius, label: "Radius")
        }
    }

    private func section(
        title: String,
        @ViewBuilder content: () -> some View
    ) -> some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
            Text(title.uppercased())
                .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                .tracking(0.6)
                .foregroundStyle(MuxyTheme.fgDim)
            content()
        }
    }

    private func propertyRow(
        _ property: StyleOverride.Property,
        label: String
    ) -> some View {
        StylePropertyField(
            session: session,
            annotationID: annotationID,
            property: property,
            originalValue: computedSeed[property.cssName] ?? "",
            label: label
        )
    }

    private func boxModelSection(
        title: String,
        top: StyleOverride.Property,
        right: StyleOverride.Property,
        bottom: StyleOverride.Property,
        left: StyleOverride.Property
    ) -> some View {
        section(title: title) {
            Grid(horizontalSpacing: UIMetrics.spacing2, verticalSpacing: UIMetrics.spacing2) {
                GridRow {
                    boxModelCell(icon: "arrow.up", property: top)
                    boxModelCell(icon: "arrow.right", property: right)
                }
                GridRow {
                    boxModelCell(icon: "arrow.down", property: bottom)
                    boxModelCell(icon: "arrow.left", property: left)
                }
            }
        }
    }

    private func boxModelCell(
        icon: String,
        property: StyleOverride.Property
    ) -> some View {
        StyleBoxModelField(
            session: session,
            annotationID: annotationID,
            property: property,
            originalValue: computedSeed[property.cssName] ?? "",
            icon: icon
        )
    }
}

private struct StylePropertyField: View {
    @Bindable var session: BrowserSession
    let annotationID: UUID
    let property: StyleOverride.Property
    let originalValue: String
    let label: String

    @State private var draftValue: String = ""

    private var existingOverride: StyleOverride? {
        session.inspector.annotations
            .first(where: { $0.id == annotationID })?
            .styleOverrides
            .first(where: { $0.property == property })
    }

    private var placeholder: String {
        originalValue.isEmpty ? "—" : originalValue
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing2) {
            Text(label)
                .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.scaled(58), alignment: .leading)
            valueField
            resetButton
        }
        .onAppear { draftValue = existingOverride?.value ?? "" }
    }

    private var valueField: some View {
        TextField(placeholder, text: $draftValue)
            .textFieldStyle(.plain)
            .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
            .foregroundStyle(MuxyTheme.fg)
            .padding(.horizontal, UIMetrics.spacing2)
            .frame(height: UIMetrics.scaled(22))
            .frame(maxWidth: .infinity)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .overlay(
                RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                    .strokeBorder(MuxyTheme.border, lineWidth: 1)
            )
            .help(placeholder)
            .onSubmit(commit)
    }

    @ViewBuilder
    private var resetButton: some View {
        if existingOverride != nil {
            Button(action: reset) {
                Image(systemName: "arrow.uturn.backward")
                    .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: UIMetrics.scaled(16), height: UIMetrics.scaled(16))
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Reset to original")
            .accessibilityLabel("Reset \(label)")
        } else {
            Color.clear.frame(width: UIMetrics.scaled(16), height: UIMetrics.scaled(16))
        }
    }

    private func reset() {
        if let override = existingOverride {
            session.removeStyleOverride(id: override.id, for: annotationID)
        }
        draftValue = ""
    }

    private func commit() {
        StyleOverrideCommitter.commit(draft: &draftValue, context: editContext)
    }

    private var editContext: StyleOverrideEditContext {
        StyleOverrideEditContext(
            session: session,
            annotationID: annotationID,
            property: property,
            originalValue: originalValue,
            existingOverride: existingOverride
        )
    }
}

private struct StyleBoxModelField: View {
    @Bindable var session: BrowserSession
    let annotationID: UUID
    let property: StyleOverride.Property
    let originalValue: String
    let icon: String

    @State private var draftValue: String = ""

    private var existingOverride: StyleOverride? {
        session.inspector.annotations
            .first(where: { $0.id == annotationID })?
            .styleOverrides
            .first(where: { $0.property == property })
    }

    private var placeholder: String {
        originalValue.isEmpty ? "—" : originalValue
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing1) {
            Image(systemName: icon)
                .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                .foregroundStyle(existingOverride == nil ? MuxyTheme.fgDim : MuxyTheme.accent)
                .frame(width: UIMetrics.scaled(14))
            TextField(placeholder, text: $draftValue)
                .textFieldStyle(.plain)
                .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                .foregroundStyle(MuxyTheme.fg)
                .padding(.horizontal, UIMetrics.spacing2)
                .frame(height: UIMetrics.scaled(22))
                .frame(maxWidth: .infinity)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                .overlay(
                    RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                        .strokeBorder(
                            existingOverride == nil ? MuxyTheme.border : MuxyTheme.accent.opacity(0.6),
                            lineWidth: 1
                        )
                )
                .help("\(property.displayName) — \(placeholder)")
                .onSubmit(commit)
                .accessibilityLabel(property.displayName)
        }
        .onAppear { draftValue = existingOverride?.value ?? "" }
    }

    private func commit() {
        StyleOverrideCommitter.commit(draft: &draftValue, context: editContext)
    }

    private var editContext: StyleOverrideEditContext {
        StyleOverrideEditContext(
            session: session,
            annotationID: annotationID,
            property: property,
            originalValue: originalValue,
            existingOverride: existingOverride
        )
    }
}

@MainActor
private struct StyleOverrideEditContext {
    let session: BrowserSession
    let annotationID: UUID
    let property: StyleOverride.Property
    let originalValue: String
    let existingOverride: StyleOverride?
}

@MainActor
private enum StyleOverrideCommitter {
    static func commit(draft: inout String, context: StyleOverrideEditContext) {
        let session = context.session
        let annotationID = context.annotationID
        guard let annotation = session.inspector.annotations.first(where: { $0.id == annotationID }) else { return }
        let trimmed = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        let sanitized = BrowserAnnotationSanitizer.sanitizeStyleValue(trimmed)
        if sanitized.isEmpty {
            if let override = context.existingOverride {
                session.removeStyleOverride(id: override.id, for: annotationID)
            }
            draft = ""
            return
        }
        draft = sanitized
        let override = StyleOverride(
            id: context.existingOverride?.id ?? UUID(),
            selector: annotation.selector,
            property: context.property,
            originalValue: context.originalValue,
            value: sanitized
        )
        session.upsertStyleOverride(override, for: annotationID)
    }
}

import SwiftUI

struct BackgroundAppearanceSliders: View {
    @Binding var transparency: Int
    @Binding var blurIntensity: Int
    let transparencyLabel: LocalizedStringResource
    let vibrancyLabel: LocalizedStringResource
    let defaultTransparency: Int
    let defaultBlurIntensity: Int

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Text(L10n.resource(transparencyLabel))
                    .font(.system(size: SettingsMetrics.labelFontSize))
                Spacer()
                Slider(
                    value: transparencyBinding,
                    in: Double(BackgroundAppearance.transparencyRange.lowerBound)
                        ... Double(BackgroundAppearance.transparencyRange.upperBound),
                    step: 1
                )
                .frame(width: 220)
                .accessibilityLabel(L10n.string(transparencyLabel))
                Text(L10n.resource("\(displayedTransparency)%"))
                    .font(.system(size: SettingsMetrics.footnoteFontSize).monospacedDigit())
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .frame(width: 34, alignment: .trailing)
            }

            HStack(spacing: 8) {
                Text(L10n.resource(vibrancyLabel))
                    .font(.system(size: SettingsMetrics.labelFontSize))
                Spacer()
                Slider(
                    value: blurIntensityBinding,
                    in: Double(BackgroundAppearance.blurIntensityRange.lowerBound)
                        ... Double(BackgroundAppearance.blurIntensityRange.upperBound),
                    step: 1
                )
                .frame(width: 220)
                .accessibilityLabel(L10n.string(vibrancyLabel))
                Text(L10n.resource("\(displayedBlurIntensity)%"))
                    .font(.system(size: SettingsMetrics.footnoteFontSize).monospacedDigit())
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .frame(width: 34, alignment: .trailing)
                Button(L10n.string("Reset")) {
                    transparency = defaultTransparency
                    blurIntensity = defaultBlurIntensity
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
            }
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }

    private var transparencyBinding: Binding<Double> {
        Binding(
            get: { Double(displayedTransparency) },
            set: { transparency = Int($0.rounded()) }
        )
    }

    private var displayedTransparency: Int {
        min(
            max(transparency, BackgroundAppearance.transparencyRange.lowerBound),
            BackgroundAppearance.transparencyRange.upperBound
        )
    }

    private var blurIntensityBinding: Binding<Double> {
        Binding(
            get: { Double(displayedBlurIntensity) },
            set: { blurIntensity = Int($0.rounded()) }
        )
    }

    private var displayedBlurIntensity: Int {
        min(
            max(blurIntensity, BackgroundAppearance.blurIntensityRange.lowerBound),
            BackgroundAppearance.blurIntensityRange.upperBound
        )
    }
}

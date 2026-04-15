import SwiftUI

struct SegmentedPicker<T: Hashable>: View {
    @Binding var selection: T
    let options: [(value: T, label: String)]

    var body: some View {
        HStack(spacing: 0) {
            ForEach(Array(options.enumerated()), id: \.offset) { index, option in
                Button {
                    selection = option.value
                } label: {
                    Text(option.label)
                        .font(.system(size: 11, weight: selection == option.value ? .semibold : .regular))
                        .foregroundStyle(selection == option.value ? MuxyTheme.fg : MuxyTheme.fgMuted)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 5)
                        .background(
                            selection == option.value
                                ? MuxyTheme.surface
                                : Color.clear,
                            in: RoundedRectangle(cornerRadius: 4)
                        )
                }
                .buttonStyle(.plain)

                if index < options.count - 1, selection != option.value,
                   selection != options[index + 1].value
                {
                    Divider()
                        .frame(height: 14)
                        .opacity(0.4)
                }
            }
        }
        .padding(2)
        .background(MuxyTheme.hover, in: RoundedRectangle(cornerRadius: 6))
        .accessibilityRepresentation {
            Picker(selection: $selection, label: EmptyView()) {
                ForEach(Array(options.enumerated()), id: \.offset) { _, option in
                    Text(option.label).tag(option.value)
                }
            }
        }
    }
}

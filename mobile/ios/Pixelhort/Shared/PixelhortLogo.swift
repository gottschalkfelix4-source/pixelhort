import SwiftUI

/// The Pixelhort mark (docs/brand/pixelhort-logo.svg), optionally with the "Pixelhort" wordmark.
struct PixelhortLogo: View {
    var size: CGFloat = 64
    var wordmark = false

    var body: some View {
        VStack(spacing: size * 0.2) {
            Image("Logo")
                .resizable()
                .interpolation(.high)
                .frame(width: size, height: size)
                .shadow(color: .black.opacity(0.12), radius: size * 0.08, y: size * 0.04)
                .accessibilityHidden(true)
            if wordmark {
                (Text(verbatim: "Pixel").fontWeight(.medium)
                    + Text(verbatim: "hort").fontWeight(.bold).foregroundColor(.accentColor))
                    .font(.system(size: size * 0.45))
                    .accessibilityLabel(Text(verbatim: "Pixelhort"))
            }
        }
    }
}

#Preview {
    PixelhortLogo(size: 72, wordmark: true)
}

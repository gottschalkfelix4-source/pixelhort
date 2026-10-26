import SwiftUI
import UIKit

/// Pinch / double-tap zoomable image. While zoomed, panning moves the image and the parent
/// pager is disabled through `isZoomed`.
struct ZoomableImage: View {
    let image: UIImage
    @Binding var isZoomed: Bool
    var onSingleTap: () -> Void = {}

    @State private var scale: CGFloat = 1
    @State private var baseScale: CGFloat = 1
    @State private var offset: CGSize = .zero
    @State private var baseOffset: CGSize = .zero

    private let maxScale: CGFloat = 6

    var body: some View {
        GeometryReader { geometry in
            Image(uiImage: image)
                .resizable()
                .scaledToFit()
                .frame(width: geometry.size.width, height: geometry.size.height)
                .scaleEffect(scale)
                .offset(offset)
                .contentShape(Rectangle())
                .simultaneousGesture(magnifyGesture)
                .gesture(panGesture(in: geometry.size), including: scale > 1.01 ? .all : .none)
                .onTapGesture(count: 2) {
                    withAnimation(.spring(duration: 0.3)) {
                        if scale > 1.01 {
                            reset()
                        } else {
                            scale = 2.5
                            baseScale = 2.5
                            isZoomed = true
                        }
                    }
                }
                .onTapGesture(count: 1) {
                    onSingleTap()
                }
        }
        .onDisappear { reset() }
    }

    private var magnifyGesture: some Gesture {
        MagnifyGesture()
            .onChanged { value in
                scale = min(max(baseScale * value.magnification, 1), maxScale)
            }
            .onEnded { _ in
                baseScale = scale
                if scale <= 1.01 {
                    withAnimation(.spring(duration: 0.25)) { reset() }
                } else {
                    isZoomed = true
                }
            }
    }

    private func panGesture(in size: CGSize) -> some Gesture {
        DragGesture()
            .onChanged { value in
                let proposed = CGSize(width: baseOffset.width + value.translation.width,
                                      height: baseOffset.height + value.translation.height)
                offset = clamp(proposed, in: size)
            }
            .onEnded { _ in
                baseOffset = offset
            }
    }

    private func clamp(_ proposed: CGSize, in size: CGSize) -> CGSize {
        let maxX = size.width * (scale - 1) / 2
        let maxY = size.height * (scale - 1) / 2
        return CGSize(width: min(max(proposed.width, -maxX), maxX),
                      height: min(max(proposed.height, -maxY), maxY))
    }

    private func reset() {
        scale = 1
        baseScale = 1
        offset = .zero
        baseOffset = .zero
        isZoomed = false
    }
}

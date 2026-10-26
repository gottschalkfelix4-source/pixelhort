import AVFoundation
import AVKit
import SwiftUI
import UIKit

/// `UIActivityViewController` for sharing downloaded files.
struct ActivityView: UIViewControllerRepresentable {
    let items: [Any]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: items, applicationActivities: nil)
    }

    func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}

/// AirPlay route picker (casting to Apple TV / AirPlay receivers).
struct AirPlayButton: UIViewRepresentable {
    var tint: UIColor = .white

    func makeUIView(context: Context) -> AVRoutePickerView {
        let view = AVRoutePickerView()
        view.prioritizesVideoDevices = true
        view.tintColor = tint
        view.activeTintColor = .systemBlue
        return view
    }

    func updateUIView(_ uiView: AVRoutePickerView, context: Context) {
        uiView.tintColor = tint
    }
}

/// Bare `AVPlayerLayer` without controls (Live Photo playback).
struct PlayerLayerView: UIViewRepresentable {
    let player: AVPlayer
    var videoGravity: AVLayerVideoGravity = .resizeAspect

    final class PlayerUIView: UIView {
        override class var layerClass: AnyClass { AVPlayerLayer.self }
        var playerLayer: AVPlayerLayer {
            // swiftlint:disable:next force_cast
            layer as! AVPlayerLayer
        }
    }

    func makeUIView(context: Context) -> PlayerUIView {
        let view = PlayerUIView()
        view.playerLayer.player = player
        view.playerLayer.videoGravity = videoGravity
        view.backgroundColor = .clear
        return view
    }

    func updateUIView(_ uiView: PlayerUIView, context: Context) {
        uiView.playerLayer.player = player
        uiView.playerLayer.videoGravity = videoGravity
    }
}

enum MediaPlayerFactory {
    /// Creates an AVPlayer for a protected server video by passing the auth header to AVURLAsset.
    static func player(url: URL, headers: [String: String]) -> AVPlayer {
        let options: [String: Any] = headers.isEmpty ? [:] : ["AVURLAssetHTTPHeaderFieldsKey": headers]
        let asset = AVURLAsset(url: url, options: options)
        let item = AVPlayerItem(asset: asset)
        let player = AVPlayer(playerItem: item)
        player.allowsExternalPlayback = true
        return player
    }
}

/// Small helper view for loading/error/empty states.
struct StateView: View {
    enum Kind {
        case loading
        case error(String)
        case empty(LocalizedStringKey, String)
    }

    let kind: Kind
    var retry: (() -> Void)?

    var body: some View {
        switch kind {
        case .loading:
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case let .error(message):
            ContentUnavailableView {
                Label("common.error", systemImage: "exclamationmark.triangle")
            } description: {
                Text(verbatim: message)
            } actions: {
                if let retry {
                    Button("common.retry", action: retry)
                }
            }
        case let .empty(title, systemImage):
            ContentUnavailableView(title, systemImage: systemImage)
        }
    }
}

extension View {
    /// Presents an error alert bound to an optional message.
    func errorAlert(_ message: Binding<String?>) -> some View {
        alert("common.error", isPresented: Binding(
            get: { message.wrappedValue != nil },
            set: { if !$0 { message.wrappedValue = nil } }
        )) {
            Button("common.ok", role: .cancel) {}
        } message: {
            Text(verbatim: message.wrappedValue ?? "")
        }
    }
}

/// Identifiable wrapper for sheet(item:) presentation of share items.
struct ShareItem: Identifiable {
    let id = UUID()
    let items: [Any]
}

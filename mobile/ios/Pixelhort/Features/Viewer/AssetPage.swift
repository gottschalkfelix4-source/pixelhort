import AVFoundation
import AVKit
import SwiftUI
import UIKit

/// One page of the viewer: zoomable photo (with Live Photo playback on long-press) or video.
struct AssetPage: View {
    let asset: AssetDto
    let isCurrent: Bool
    @Binding var isZoomed: Bool
    var onTap: () -> Void

    @Environment(AppSession.self) private var session
    @State private var image: UIImage?
    @State private var player: AVPlayer?
    @State private var livePlayer: AVPlayer?
    @State private var isPlayingLive = false
    @State private var liveEndObserver: NSObjectProtocol?

    var body: some View {
        ZStack {
            if asset.isVideo {
                videoContent
            } else {
                imageContent
            }
        }
        .task(id: asset.id) { await loadImage() }
        .onChange(of: isCurrent) { _, current in
            if !current {
                player?.pause()
                stopLive()
            }
        }
        .onDisappear {
            player?.pause()
            player = nil
            stopLive()
            if let liveEndObserver {
                NotificationCenter.default.removeObserver(liveEndObserver)
                self.liveEndObserver = nil
            }
            livePlayer = nil
        }
    }

    // MARK: Photo

    @ViewBuilder
    private var imageContent: some View {
        if let image {
            ZoomableImage(image: image, isZoomed: $isZoomed, onSingleTap: onTap)
                .overlay {
                    if isPlayingLive, let livePlayer {
                        PlayerLayerView(player: livePlayer)
                            .allowsHitTesting(false)
                            .transition(.opacity)
                    }
                }
                .onLongPressGesture(minimumDuration: 0.25, maximumDistance: 20) {
                    startLive()
                } onPressingChanged: { pressing in
                    if !pressing { stopLive() }
                }
                .overlay(alignment: .topLeading) {
                    if asset.isLivePhoto && !isZoomed {
                        Label("viewer.live", systemImage: "livephoto")
                            .font(.caption.bold())
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(.ultraThinMaterial, in: Capsule())
                            .padding(.top, 110)
                            .padding(.leading, 12)
                            .opacity(isPlayingLive ? 0.4 : 1)
                    }
                }
        } else {
            ProgressView()
                .tint(.white)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .contentShape(Rectangle())
                .onTapGesture(perform: onTap)
        }
    }

    private func startLive() {
        guard let videoId = asset.livePhotoVideoId, let api = session.api else { return }
        let livePlayer = self.livePlayer ?? MediaPlayerFactory.player(url: api.videoURL(assetId: videoId),
                                                                      headers: api.authHeaders)
        self.livePlayer = livePlayer
        livePlayer.isMuted = false
        livePlayer.seek(to: .zero)
        if liveEndObserver == nil, let item = livePlayer.currentItem {
            liveEndObserver = NotificationCenter.default.addObserver(
                forName: AVPlayerItem.didPlayToEndTimeNotification, object: item, queue: .main
            ) { _ in
                Task { @MainActor in stopLive() }
            }
        }
        livePlayer.play()
        withAnimation(.easeIn(duration: 0.15)) { isPlayingLive = true }
    }

    private func stopLive() {
        livePlayer?.pause()
        if isPlayingLive {
            withAnimation(.easeOut(duration: 0.2)) { isPlayingLive = false }
        }
    }

    // MARK: Video

    @ViewBuilder
    private var videoContent: some View {
        if let player {
            VideoPlayer(player: player)
                .onAppear { player.play() }
        } else {
            ZStack {
                if let image {
                    Image(uiImage: image)
                        .resizable()
                        .scaledToFit()
                }
                Button {
                    startVideo()
                } label: {
                    Image(systemName: "play.circle.fill")
                        .font(.system(size: 72))
                        .symbolRenderingMode(.palette)
                        .foregroundStyle(Color.white, Color.black.opacity(0.4))
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .contentShape(Rectangle())
            .onTapGesture(perform: onTap)
        }
    }

    private func startVideo() {
        guard let api = session.api else { return }
        let player = MediaPlayerFactory.player(url: api.videoURL(assetId: asset.id), headers: api.authHeaders)
        self.player = player
        player.play()
    }

    // MARK: Loading

    private func loadImage() async {
        let previewKey = ImageLoader.assetKey(asset.id, .preview)
        if let cached = ImageLoader.shared.cachedImage(forKey: previewKey) {
            image = cached
            return
        }
        if let thumb = ImageLoader.shared.cachedImage(forKey: ImageLoader.assetKey(asset.id, .thumbnail)) {
            image = thumb
        }
        guard let api = session.api else { return }
        if let preview = await ImageLoader.shared.assetImage(asset.id, size: .preview, api: api), !Task.isCancelled {
            image = preview
        }
    }
}

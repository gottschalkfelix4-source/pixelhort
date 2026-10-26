import AVKit
import SwiftUI
import UIKit

/// Casting via `POST /api/assets/{id}/cast-url`: the returned URLs carry a `castToken`, so
/// receivers (AirPlay devices, TVs, other browsers) can fetch the media without our auth header.
struct CastSheet: View {
    let asset: AssetDto

    @Environment(AppSession.self) private var session
    @Environment(\.dismiss) private var dismiss
    @State private var cast: CastURLResponse?
    @State private var player: AVPlayer?
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            Group {
                if let cast {
                    content(cast)
                } else if let errorMessage {
                    StateView(kind: .error(errorMessage)) { Task { await load() } }
                } else {
                    StateView(kind: .loading)
                }
            }
            .navigationTitle("cast.title")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("common.done") { dismiss() }
                }
                ToolbarItem(placement: .topBarLeading) {
                    AirPlayButton(tint: .systemBlue)
                        .frame(width: 32, height: 32)
                }
            }
        }
        .task { await load() }
        .onDisappear { player?.pause() }
    }

    private var hintKey: LocalizedStringKey {
        asset.isVideo ? "cast.video.hint" : "cast.photo.hint"
    }

    @ViewBuilder
    private func content(_ cast: CastURLResponse) -> some View {
        List {
            Section {
                if asset.isVideo, let player {
                    VideoPlayer(player: player)
                        .frame(height: 240)
                        .listRowInsets(EdgeInsets())
                } else if let imageURL = cast.imageUrl.flatMap(URL.init(string:)) {
                    AsyncImage(url: imageURL) { image in
                        image.resizable().scaledToFit()
                    } placeholder: {
                        ProgressView().frame(maxWidth: .infinity, minHeight: 200)
                    }
                    .listRowInsets(EdgeInsets())
                }
            } footer: {
                Text(hintKey)
            }

            Section {
                if let url = URL(string: asset.isVideo ? cast.url : (cast.imageUrl ?? cast.url)) {
                    ShareLink(item: url) {
                        Label("cast.shareLink", systemImage: "square.and.arrow.up")
                    }
                    Button {
                        UIPasteboard.general.url = url
                    } label: {
                        Label("cast.copyLink", systemImage: "doc.on.doc")
                    }
                }
            } footer: {
                Text("cast.validity")
            }

            Section {
                Text("cast.googleCast.hint")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private func load() async {
        guard let api = session.api else { return }
        do {
            let response = try await api.castURL(assetId: asset.id)
            cast = response
            errorMessage = nil
            if asset.isVideo, let url = URL(string: response.url) {
                // castToken URL: AirPlay receivers fetch it directly (no auth header needed).
                let player = AVPlayer(url: url)
                player.allowsExternalPlayback = true
                player.usesExternalPlaybackWhileExternalScreenIsActive = true
                self.player = player
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

import SwiftUI

/// The main "Fotos" tab: memories carousel + full timeline.
struct TimelineScreen: View {
    @Environment(AppSession.self) private var session
    @Environment(BackupService.self) private var backup
    @AppStorage("timeline.withPartners") private var withPartners = false
    @State private var showBackup = false

    var body: some View {
        AssetTimelineView(context: .timeline(withPartners: withPartners)) {
            if session.preferences.memories.enabled {
                MemoriesCarousel()
            }
        }
        .navigationTitle("tab.photos")
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                Button {
                    showBackup = true
                } label: {
                    BackupStatusIcon(status: backup.status, isRunning: backup.isRunning)
                }
                .accessibilityLabel(Text("backup.title"))
            }
        }
        .sheet(isPresented: $showBackup) {
            NavigationStack {
                BackupProgressView()
                    .toolbar {
                        ToolbarItem(placement: .confirmationAction) {
                            Button("common.done") { showBackup = false }
                        }
                    }
            }
        }
    }
}

struct BackupStatusIcon: View {
    let status: BackupService.Status
    let isRunning: Bool

    var body: some View {
        switch status {
        case .disabled:
            Image(systemName: "icloud.slash")
        case .noPermission, .failed:
            Image(systemName: "exclamationmark.icloud")
                .foregroundStyle(.orange)
        case .waitingForWiFi:
            Image(systemName: "wifi.exclamationmark")
        case .scanning, .uploading:
            Image(systemName: "arrow.triangle.2.circlepath.icloud")
                .symbolEffect(.pulse, isActive: isRunning)
        case .idle:
            Image(systemName: "checkmark.icloud")
        }
    }
}

/// Horizontal "On this day" carousel from `/api/memories`.
struct MemoriesCarousel: View {
    @Environment(AppSession.self) private var session
    @State private var memories: [Memory] = []
    @State private var viewer: ViewerRequest?

    var body: some View {
        // A placeholder keeps the view alive while empty: an empty Group never appears inside the
        // timeline's LazyVStack, so its `.task` would never load the memories.
        Group {
            if memories.isEmpty {
                Color.clear.frame(height: 0)
            } else {
                ScrollView(.horizontal, showsIndicators: false) {
                    LazyHStack(spacing: 10) {
                        ForEach(memories) { memory in
                            if let first = memory.assets.first {
                                Button {
                                    viewer = ViewerRequest(assets: memory.assets, startId: first.id)
                                } label: {
                                    MemoryCard(memory: memory, coverId: first.id)
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }
                    .padding(.horizontal)
                    .padding(.vertical, 8)
                }
            }
        }
        .task {
            guard let api = session.api, memories.isEmpty else { return }
            memories = ((try? await api.memories()) ?? []).filter { !$0.assets.isEmpty }
        }
        .fullScreenCover(item: $viewer) { request in
            AssetViewer(request: request) {}
        }
    }
}

private struct MemoryCard: View {
    let memory: Memory
    let coverId: String

    var body: some View {
        AssetThumbnail(assetId: coverId, size: .preview)
            .frame(width: 120, height: 170)
            .overlay {
                LinearGradient(colors: [.clear, .black.opacity(0.45)], startPoint: .center, endPoint: .bottom)
                    .allowsHitTesting(false)
            }
            .clipShape(RoundedRectangle(cornerRadius: 14))
            .overlay(alignment: .bottomLeading) {
                Group {
                    if let years = memory.yearsAgo {
                        Text("memories.yearsAgo \(years)")
                    } else {
                        Text(verbatim: memory.title ?? "")
                    }
                }
                .font(.footnote.bold())
                .foregroundStyle(.white)
                .shadow(radius: 3)
                .padding(8)
            }
    }
}

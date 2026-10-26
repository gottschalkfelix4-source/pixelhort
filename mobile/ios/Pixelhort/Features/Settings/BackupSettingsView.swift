import Photos
import SwiftUI
import UIKit

struct BackupSettingsView: View {
    @Environment(BackupService.self) private var backup
    @State private var authorization: PHAuthorizationStatus = .notDetermined
    @State private var confirmReset = false

    var body: some View {
        @Bindable var backup = backup
        Form {
            Section {
                Toggle("backup.enabled", isOn: Binding(
                    get: { backup.settings.isEnabled },
                    set: { newValue in
                        if newValue {
                            Task {
                                if await backup.requestAuthorization() {
                                    backup.settings.isEnabled = true
                                }
                                authorization = backup.authorizationStatus
                            }
                        } else {
                            backup.settings.isEnabled = false
                        }
                    }
                ))
                .accessibilityIdentifier("backup.enabledToggle")
            } footer: {
                Text("backup.enabled.footer")
            }

            if authorization == .denied || authorization == .restricted {
                Section {
                    Label("backup.permission.denied", systemImage: "exclamationmark.triangle")
                        .foregroundStyle(.orange)
                    Button("backup.permission.openSettings") {
                        if let url = URL(string: UIApplication.openSettingsURLString) {
                            UIApplication.shared.open(url)
                        }
                    }
                }
            } else if authorization == .limited {
                Section {
                    Label("backup.permission.limited", systemImage: "info.circle")
                        .foregroundStyle(.secondary)
                }
            }

            Section("backup.source") {
                Toggle("backup.allPhotos", isOn: $backup.settings.backupAllPhotos)
                if !backup.settings.backupAllPhotos {
                    NavigationLink {
                        AlbumSelectionView()
                    } label: {
                        LabeledContent("backup.albums") {
                            Text("backup.albums.count \(backup.settings.selectedAlbumIds.count)")
                        }
                    }
                }
            }

            Section {
                Toggle("backup.wifiOnly", isOn: $backup.settings.wifiOnly)
                Toggle("backup.includeVideos", isOn: $backup.settings.includeVideos)
                Toggle("backup.background", isOn: $backup.settings.backgroundBackup)
            } header: {
                Text("backup.options")
            } footer: {
                Text("backup.background.footer")
            }

            Section {
                Button("backup.reset", role: .destructive) { confirmReset = true }
            } footer: {
                Text("backup.reset.footer")
            }
        }
        .navigationTitle("settings.backup.options")
        .onAppear { authorization = backup.authorizationStatus }
        .onChange(of: backup.settings.backgroundBackup) { _, _ in
            BackgroundTaskScheduler.schedule()
        }
        .confirmationDialog("backup.reset.confirm", isPresented: $confirmReset, titleVisibility: .visible) {
            Button("backup.reset", role: .destructive) {
                Task {
                    await backup.resetState()
                    backup.start(.manual)
                }
            }
        }
    }
}

/// Choose which local albums (`PHAssetCollection`) are backed up.
struct AlbumSelectionView: View {
    struct LocalAlbum: Identifiable {
        let id: String
        let title: String
        let count: Int
        let isSmart: Bool
    }

    @Environment(BackupService.self) private var backup
    @State private var albums: [LocalAlbum] = []

    var body: some View {
        List {
            Section("backup.albums.smart") {
                ForEach(albums.filter(\.isSmart)) { album in row(album) }
            }
            Section("backup.albums.user") {
                ForEach(albums.filter { !$0.isSmart }) { album in row(album) }
            }
        }
        .navigationTitle("backup.albums")
        .task { albums = Self.loadAlbums() }
    }

    private func row(_ album: LocalAlbum) -> some View {
        let isSelected = backup.settings.selectedAlbumIds.contains(album.id)
        return Button {
            var ids = backup.settings.selectedAlbumIds
            if isSelected {
                ids.removeAll { $0 == album.id }
            } else {
                ids.append(album.id)
            }
            backup.settings.selectedAlbumIds = ids
        } label: {
            HStack {
                VStack(alignment: .leading) {
                    Text(verbatim: album.title).foregroundStyle(.primary)
                    Text(verbatim: "\(album.count)").font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                if isSelected {
                    Image(systemName: "checkmark").foregroundStyle(.tint)
                }
            }
        }
    }

    private static func loadAlbums() -> [LocalAlbum] {
        var result: [LocalAlbum] = []
        let countOptions = PHFetchOptions()
        countOptions.includeHiddenAssets = false

        let smartTypes: [PHAssetCollectionSubtype] = [
            .smartAlbumUserLibrary, .smartAlbumFavorites, .smartAlbumSelfPortraits,
            .smartAlbumLivePhotos, .smartAlbumVideos, .smartAlbumScreenshots, .smartAlbumPanoramas,
        ]
        for subtype in smartTypes {
            let fetch = PHAssetCollection.fetchAssetCollections(with: .smartAlbum, subtype: subtype, options: nil)
            fetch.enumerateObjects { collection, _, _ in
                let count = PHAsset.fetchAssets(in: collection, options: countOptions).count
                result.append(LocalAlbum(id: collection.localIdentifier,
                                         title: collection.localizedTitle ?? "–",
                                         count: count, isSmart: true))
            }
        }

        let userAlbums = PHAssetCollection.fetchAssetCollections(with: .album, subtype: .any, options: nil)
        userAlbums.enumerateObjects { collection, _, _ in
            let count = PHAsset.fetchAssets(in: collection, options: countOptions).count
            result.append(LocalAlbum(id: collection.localIdentifier,
                                     title: collection.localizedTitle ?? "–",
                                     count: count, isSmart: false))
        }
        return result
    }
}

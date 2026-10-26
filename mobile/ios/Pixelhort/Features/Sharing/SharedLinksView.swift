import SwiftUI
import UIKit

struct SharedLinksView: View {
    @Environment(AppSession.self) private var session
    @State private var links: [SharedLinkDto] = []
    @State private var isLoading = false
    @State private var errorMessage: String?

    var body: some View {
        List {
            ForEach(links) { link in
                SharedLinkRow(link: link)
                    .swipeActions {
                        Button(role: .destructive) {
                            Task { await delete(link) }
                        } label: {
                            Label("common.delete", systemImage: "trash")
                        }
                    }
            }
        }
        .overlay {
            if isLoading && links.isEmpty {
                ProgressView()
            } else if !isLoading && links.isEmpty {
                StateView(kind: .empty("sharedLinks.empty", "link"))
            }
        }
        .navigationTitle("library.sharedLinks")
        .task { await load() }
        .refreshable { await load() }
        .errorAlert($errorMessage)
    }

    private func load() async {
        guard let api = session.api else { return }
        isLoading = true
        defer { isLoading = false }
        do {
            links = try await api.sharedLinks()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func delete(_ link: SharedLinkDto) async {
        guard let api = session.api else { return }
        do {
            try await api.deleteSharedLink(id: link.id)
            links.removeAll { $0.id == link.id }
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

struct SharedLinkRow: View {
    let link: SharedLinkDto

    private var coverId: String? {
        link.album?.albumThumbnailAssetId ?? link.assetIds?.first
    }

    var body: some View {
        HStack(spacing: 12) {
            Group {
                if let coverId {
                    AssetThumbnail(assetId: coverId)
                } else {
                    Rectangle().fill(Color.secondary.opacity(0.15))
                }
            }
            .frame(width: 52, height: 52)
            .clipShape(RoundedRectangle(cornerRadius: 8))

            VStack(alignment: .leading, spacing: 3) {
                if let album = link.album {
                    Text(verbatim: album.albumName).font(.headline)
                } else {
                    Text("sharedLinks.assets \(link.assetIds?.count ?? 0)").font(.headline)
                }
                if let description = link.description, !description.isEmpty {
                    Text(verbatim: description).font(.subheadline).foregroundStyle(.secondary)
                }
                HStack(spacing: 6) {
                    if link.hasPassword == true { Image(systemName: "lock.fill") }
                    if link.isExpired {
                        Text("sharedLinks.expired").foregroundStyle(.red)
                    } else if let expires = link.expiresAt {
                        Text("sharedLinks.expires \(expires.formatted(date: .abbreviated, time: .omitted))")
                    }
                }
                .font(.caption)
                .foregroundStyle(.secondary)
            }
            Spacer()
            if let urlString = link.url, let url = URL(string: urlString) {
                ShareLink(item: url) {
                    Image(systemName: "square.and.arrow.up")
                }
                .buttonStyle(.borderless)
            }
        }
        .contextMenu {
            if let urlString = link.url, let url = URL(string: urlString) {
                Button {
                    UIPasteboard.general.url = url
                } label: {
                    Label("cast.copyLink", systemImage: "doc.on.doc")
                }
            }
        }
    }
}

/// Create a public shared link for assets or an album.
struct CreateSharedLinkSheet: View {
    let type: SharedLinkType
    let assetIds: [String]
    let albumId: String?

    enum Expiry: String, CaseIterable, Identifiable {
        case never, day, week, month
        var id: String { rawValue }

        var title: LocalizedStringKey {
            switch self {
            case .never: return "sharedLinks.expiry.never"
            case .day: return "sharedLinks.expiry.day"
            case .week: return "sharedLinks.expiry.week"
            case .month: return "sharedLinks.expiry.month"
            }
        }

        var date: Date? {
            switch self {
            case .never: return nil
            case .day: return Date().addingTimeInterval(86_400)
            case .week: return Date().addingTimeInterval(7 * 86_400)
            case .month: return Date().addingTimeInterval(30 * 86_400)
            }
        }
    }

    @Environment(AppSession.self) private var session
    @Environment(\.dismiss) private var dismiss
    @State private var description = ""
    @State private var password = ""
    @State private var expiry: Expiry = .never
    @State private var allowDownload = true
    @State private var allowUpload = false
    @State private var showMetadata = true
    @State private var created: SharedLinkDto?
    @State private var isSaving = false
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            Form {
                if let created, let urlString = created.url, let url = URL(string: urlString) {
                    Section("sharedLinks.created") {
                        Text(verbatim: urlString)
                            .font(.footnote.monospaced())
                            .textSelection(.enabled)
                        ShareLink(item: url) {
                            Label("action.share", systemImage: "square.and.arrow.up")
                        }
                        Button {
                            UIPasteboard.general.url = url
                        } label: {
                            Label("cast.copyLink", systemImage: "doc.on.doc")
                        }
                    }
                } else {
                    Section {
                        if type == .individual {
                            Text("sharedLinks.assets \(assetIds.count)")
                                .foregroundStyle(.secondary)
                        }
                        TextField("sharedLinks.description", text: $description)
                        SecureField("sharedLinks.password", text: $password)
                        Picker("sharedLinks.expiry", selection: $expiry) {
                            ForEach(Expiry.allCases) { Text($0.title).tag($0) }
                        }
                    }
                    Section {
                        Toggle("sharedLinks.allowDownload", isOn: $allowDownload)
                        if type == .album {
                            Toggle("sharedLinks.allowUpload", isOn: $allowUpload)
                        }
                        Toggle("sharedLinks.showMetadata", isOn: $showMetadata)
                    }
                }
            }
            .navigationTitle("action.sharedLink")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button { dismiss() } label: { Text(closeTitle) }
                }
                ToolbarItem(placement: .confirmationAction) {
                    if created == nil {
                        if isSaving {
                            ProgressView()
                        } else {
                            Button("common.create") { Task { await create() } }
                        }
                    }
                }
            }
            .errorAlert($errorMessage)
        }
    }

    private var closeTitle: LocalizedStringKey {
        created == nil ? "common.cancel" : "common.done"
    }

    private func create() async {
        guard let api = session.api else { return }
        isSaving = true
        defer { isSaving = false }
        do {
            created = try await api.createSharedLink(CreateSharedLinkRequest(
                type: type,
                albumId: albumId,
                assetIds: type == .individual ? assetIds : nil,
                description: description.isEmpty ? nil : description,
                password: password.isEmpty ? nil : password,
                expiresAt: expiry.date,
                allowDownload: allowDownload,
                allowUpload: type == .album ? allowUpload : nil,
                showMetadata: showMetadata))
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

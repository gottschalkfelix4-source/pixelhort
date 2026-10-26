import SwiftUI

/// "Bibliothek" tab: entry points to filtered views and sharing features.
struct LibraryScreen: View {
    var body: some View {
        List {
            Section {
                NavigationLink {
                    AssetTimelineView(context: .favorites)
                        .navigationTitle("library.favorites")
                } label: {
                    Label("library.favorites", systemImage: "heart")
                }
                NavigationLink {
                    AssetTimelineView(context: .archive)
                        .navigationTitle("library.archive")
                } label: {
                    Label("library.archive", systemImage: "archivebox")
                }
                NavigationLink {
                    LockedFolderView()
                } label: {
                    Label("library.locked", systemImage: "lock.shield")
                }
                NavigationLink {
                    TrashView()
                } label: {
                    Label("library.trash", systemImage: "trash")
                }
            }

            Section("library.section.explore") {
                NavigationLink {
                    PeopleView()
                } label: {
                    Label("library.people", systemImage: "person.2")
                }
                NavigationLink {
                    MapScreen()
                } label: {
                    Label("library.map", systemImage: "map")
                }
            }

            Section("library.section.sharing") {
                NavigationLink {
                    SharedLinksView()
                } label: {
                    Label("library.sharedLinks", systemImage: "link")
                }
                NavigationLink {
                    PartnersView()
                } label: {
                    Label("library.partners", systemImage: "person.2.badge.gearshape")
                }
            }
        }
        .navigationTitle("tab.library")
    }
}

/// Trash with restore-all / empty actions.
struct TrashView: View {
    @Environment(AppSession.self) private var session
    @State private var confirmEmpty = false
    @State private var reloadToken = UUID()
    @State private var errorMessage: String?

    var body: some View {
        AssetTimelineView(context: .trash) {
            Text("trash.hint")
                .font(.footnote)
                .foregroundStyle(.secondary)
                .padding()
        }
        .id(reloadToken)
        .navigationTitle("library.trash")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    Button {
                        Task { await restoreAll() }
                    } label: {
                        Label("trash.restoreAll", systemImage: "arrow.uturn.backward")
                    }
                    Button(role: .destructive) {
                        confirmEmpty = true
                    } label: {
                        Label("trash.empty", systemImage: "trash.slash")
                    }
                } label: {
                    Image(systemName: "ellipsis.circle")
                }
            }
        }
        .confirmationDialog("trash.empty.confirm", isPresented: $confirmEmpty, titleVisibility: .visible) {
            Button("trash.empty", role: .destructive) { Task { await emptyTrash() } }
        }
        .errorAlert($errorMessage)
    }

    private func restoreAll() async {
        guard let api = session.api else { return }
        do {
            try await api.restoreTrash(ids: nil)
            reloadToken = UUID()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func emptyTrash() async {
        guard let api = session.api else { return }
        do {
            try await api.emptyTrash()
            reloadToken = UUID()
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

import SwiftUI

struct RootView: View {
    @Environment(AppSession.self) private var session
    @Environment(BackupService.self) private var backup
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        Group {
            switch session.phase {
            case .serverSetup:
                NavigationStack {
                    ServerSetupView()
                }
            case .login:
                NavigationStack {
                    LoginView()
                }
            case .ready:
                MainTabView()
                    .task {
                        await session.refresh()
                        backup.startObservingLibrary()
                        backup.start(.foreground)
                    }
            }
        }
        .onChange(of: scenePhase) { _, newPhase in
            switch newPhase {
            case .active:
                if session.phase == .ready {
                    backup.start(.foreground)
                }
            case .background:
                BackgroundTaskScheduler.schedule()
                Task { await UploadStateStore.shared.flush() }
            default:
                break
            }
        }
    }
}

struct MainTabView: View {
    var body: some View {
        TabView {
            NavigationStack { TimelineScreen() }
                .tabItem { Label("tab.photos", systemImage: "photo.on.rectangle") }
            NavigationStack { SearchScreen() }
                .tabItem { Label("tab.search", systemImage: "magnifyingglass") }
            NavigationStack { AlbumsScreen() }
                .tabItem { Label("tab.albums", systemImage: "rectangle.stack") }
            NavigationStack { LibraryScreen() }
                .tabItem { Label("tab.library", systemImage: "books.vertical") }
            NavigationStack { SettingsScreen() }
                .tabItem { Label("tab.settings", systemImage: "gearshape") }
        }
    }
}

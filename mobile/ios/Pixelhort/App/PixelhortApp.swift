import SwiftUI

@main
@MainActor
struct PixelhortApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @State private var session = AppSession.shared
    @State private var backup = BackupService.shared

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(session)
                .environment(backup)
        }
    }
}

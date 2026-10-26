import UIKit

final class AppDelegate: NSObject, UIApplicationDelegate {
    func application(_ application: UIApplication,
                     didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        BackgroundTaskScheduler.register()
        BackupUploader.shared.activate()
        return true
    }

    /// Called when the background upload session has events while the app was not running.
    func application(_ application: UIApplication,
                     handleEventsForBackgroundURLSession identifier: String,
                     completionHandler: @escaping () -> Void) {
        guard identifier == BackupUploader.sessionIdentifier else {
            completionHandler()
            return
        }
        BackupUploader.shared.backgroundCompletionHandler = completionHandler
        BackupUploader.shared.activate()
    }
}

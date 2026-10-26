import BackgroundTasks
import Foundation

/// Registers and schedules the BGTaskScheduler tasks that look for new photos.
/// Identifiers must match `BGTaskSchedulerPermittedIdentifiers` in Info.plist (project.yml).
enum BackgroundTaskScheduler {
    static let refreshIdentifier = "app.pixelhort.backup.refresh"
    static let processingIdentifier = "app.pixelhort.backup.processing"

    /// Must be called before the end of `application(_:didFinishLaunchingWithOptions:)`.
    static func register() {
        BGTaskScheduler.shared.register(forTaskWithIdentifier: refreshIdentifier, using: nil) { task in
            guard let task = task as? BGAppRefreshTask else { return }
            handle(task, trigger: .backgroundRefresh)
        }
        BGTaskScheduler.shared.register(forTaskWithIdentifier: processingIdentifier, using: nil) { task in
            guard let task = task as? BGProcessingTask else { return }
            handle(task, trigger: .backgroundProcessing)
        }
    }

    static func schedule() {
        let settings = BackupSettings.load()
        guard settings.isEnabled, settings.backgroundBackup else {
            BGTaskScheduler.shared.cancelAllTaskRequests()
            return
        }

        let refresh = BGAppRefreshTaskRequest(identifier: refreshIdentifier)
        refresh.earliestBeginDate = Date(timeIntervalSinceNow: 15 * 60)
        try? BGTaskScheduler.shared.submit(refresh)

        let processing = BGProcessingTaskRequest(identifier: processingIdentifier)
        processing.requiresNetworkConnectivity = true
        processing.requiresExternalPower = false
        processing.earliestBeginDate = Date(timeIntervalSinceNow: 60 * 60)
        try? BGTaskScheduler.shared.submit(processing)
    }

    private static func handle(_ task: BGTask, trigger: BackupService.Trigger) {
        // Always chain the next request first.
        schedule()
        let work = Task { @MainActor in
            await AppSession.shared.refresh()
            await BackupService.shared.runToCompletion(trigger)
            task.setTaskCompleted(success: !Task.isCancelled)
        }
        task.expirationHandler = {
            work.cancel()
            Task { @MainActor in
                BackupService.shared.stop()
            }
        }
    }
}

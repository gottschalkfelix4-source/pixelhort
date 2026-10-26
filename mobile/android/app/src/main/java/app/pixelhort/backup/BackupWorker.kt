package app.pixelhort.backup

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.provider.MediaStore
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.Data
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.ForegroundInfo
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.OutOfQuotaPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkInfo
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import app.pixelhort.MainActivity
import app.pixelhort.PixelhortApp
import app.pixelhort.R
import app.pixelhort.data.settings.BackupOptions
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import java.util.concurrent.TimeUnit

/** Runs one backup pass. Promotes itself to a foreground (dataSync) service for long runs. */
class BackupWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {

    private val container get() = (applicationContext as PixelhortApp).container
    private var foregroundActive = false
    private var lastNotify = 0L

    override suspend fun doWork(): Result {
        val engine = container.backupEngine
        if (!container.settings.current.isLoggedIn) return Result.success()
        BackupNotifications.ensureChannels(applicationContext)

        val result = try {
            engine.run { p ->
                setProgress(
                    workDataOf(
                        KEY_DONE to p.done, KEY_TOTAL to p.total, KEY_FILE to (p.currentFile ?: ""),
                        KEY_UPLOADED to p.uploaded, KEY_SKIPPED to p.skipped, KEY_FAILED to p.failed,
                    )
                )
                val remaining = p.total - p.done
                if (!foregroundActive && remaining > FOREGROUND_THRESHOLD) {
                    foregroundActive = true
                    runCatching { setForeground(foregroundInfo(p)) }
                        .onFailure { Log.w(TAG, "cannot start foreground service", it) }
                } else if (foregroundActive && System.currentTimeMillis() - lastNotify > 1000) {
                    lastNotify = System.currentTimeMillis()
                    runCatching { setForeground(foregroundInfo(p)) }
                }
            }
        } catch (e: Exception) {
            if (e is kotlinx.coroutines.CancellationException) throw e
            Log.e(TAG, "backup failed", e)
            return if (runAttemptCount < 5) Result.retry() else Result.failure()
        }

        if (result.uploaded > 0 || result.failed > 0) BackupNotifications.notifyDone(applicationContext, result)
        val out = workDataOf(
            KEY_UPLOADED to result.uploaded, KEY_SKIPPED to result.skipped,
            KEY_FAILED to result.failed, KEY_TOTAL to result.total,
        )
        return when {
            result.retry && runAttemptCount < 5 -> Result.retry()
            else -> Result.success(out)
        }
    }

    override suspend fun getForegroundInfo(): ForegroundInfo =
        foregroundInfo(BackupProgress(0, 0, null, 0, 0, 0))

    private fun foregroundInfo(p: BackupProgress): ForegroundInfo {
        BackupNotifications.ensureChannels(applicationContext)
        val n = BackupNotifications.progressNotification(applicationContext, p, id)
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            ForegroundInfo(BackupNotifications.PROGRESS_ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        } else {
            ForegroundInfo(BackupNotifications.PROGRESS_ID, n)
        }
    }

    companion object {
        private const val TAG = "BackupWorker"
        const val KEY_DONE = "done"
        const val KEY_TOTAL = "total"
        const val KEY_FILE = "file"
        const val KEY_UPLOADED = "uploaded"
        const val KEY_SKIPPED = "skipped"
        const val KEY_FAILED = "failed"
        const val KEY_FROM_TRIGGER = "fromTrigger"
        private const val FOREGROUND_THRESHOLD = 10
    }
}

/**
 * Fires when MediaStore content changes (new photo/video). Starts a constrained backup run;
 * the backup worker re-arms this trigger afterwards (content-URI triggers are one-shot).
 */
class MediaTriggerWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val settings = (applicationContext as PixelhortApp).container.settings.current
        if (settings.isLoggedIn && settings.backup.enabled) {
            BackupScheduler.enqueueRun(applicationContext, settings.backup, fromTrigger = true)
            // Content-URI triggers are one-shot: re-arm (appended, so it starts after this worker ends).
            BackupScheduler.scheduleContentTrigger(applicationContext, settings.backup, ExistingWorkPolicy.APPEND_OR_REPLACE)
        }
        return Result.success()
    }
}

object BackupScheduler {
    const val WORK_RUN = "pixelhort-backup-run"
    const val WORK_MANUAL = "pixelhort-backup-manual"
    const val WORK_PERIODIC = "pixelhort-backup-periodic"
    const val WORK_TRIGGER = "pixelhort-backup-trigger"

    private fun constraints(o: BackupOptions): Constraints = Constraints.Builder()
        .setRequiredNetworkType(if (o.wifiOnly) NetworkType.UNMETERED else NetworkType.CONNECTED)
        .setRequiresCharging(o.chargingOnly)
        .setRequiresBatteryNotLow(true)
        .build()

    /** Applies the user's background settings: (re)schedules or cancels periodic + trigger work. */
    fun apply(context: Context, o: BackupOptions, loggedIn: Boolean) {
        val wm = WorkManager.getInstance(context)
        if (!loggedIn || !o.enabled || o.selectedBuckets.isEmpty()) {
            wm.cancelUniqueWork(WORK_PERIODIC)
            wm.cancelUniqueWork(WORK_TRIGGER)
            wm.cancelUniqueWork(WORK_RUN)
            return
        }
        val periodic = PeriodicWorkRequestBuilder<BackupWorker>(1, TimeUnit.HOURS, 15, TimeUnit.MINUTES)
            .setConstraints(constraints(o))
            .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 1, TimeUnit.MINUTES)
            .build()
        wm.enqueueUniquePeriodicWork(WORK_PERIODIC, ExistingPeriodicWorkPolicy.UPDATE, periodic)
        scheduleContentTrigger(context, o)
    }

    fun scheduleContentTrigger(context: Context, o: BackupOptions, policy: ExistingWorkPolicy = ExistingWorkPolicy.REPLACE) {
        if (!o.enabled) return
        val req = OneTimeWorkRequestBuilder<MediaTriggerWorker>()
            .setConstraints(
                Constraints.Builder()
                    .addContentUriTrigger(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, true)
                    .apply { if (o.includeVideos) addContentUriTrigger(MediaStore.Video.Media.EXTERNAL_CONTENT_URI, true) }
                    .setTriggerContentUpdateDelay(10, TimeUnit.SECONDS)
                    .setTriggerContentMaxDelay(2, TimeUnit.MINUTES)
                    .build()
            )
            .build()
        WorkManager.getInstance(context).enqueueUniqueWork(WORK_TRIGGER, policy, req)
    }

    /** Constrained background run (wifi/charging per settings). */
    fun enqueueRun(context: Context, o: BackupOptions, fromTrigger: Boolean) {
        val req = OneTimeWorkRequestBuilder<BackupWorker>()
            .setConstraints(constraints(o))
            .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 1, TimeUnit.MINUTES)
            .setInputData(Data.Builder().putBoolean(BackupWorker.KEY_FROM_TRIGGER, fromTrigger).build())
            .build()
        WorkManager.getInstance(context).enqueueUniqueWork(WORK_RUN, ExistingWorkPolicy.KEEP, req)
    }

    /** "Jetzt sichern": expedited, ignores wifi/charging preferences (only needs a network). */
    fun backupNow(context: Context) {
        val req = OneTimeWorkRequestBuilder<BackupWorker>()
            .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
            .setExpedited(OutOfQuotaPolicy.RUN_AS_NON_EXPEDITED_WORK_REQUEST)
            .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
            .build()
        WorkManager.getInstance(context).enqueueUniqueWork(WORK_MANUAL, ExistingWorkPolicy.KEEP, req)
    }

    fun cancelAll(context: Context) {
        val wm = WorkManager.getInstance(context)
        listOf(WORK_RUN, WORK_MANUAL, WORK_PERIODIC, WORK_TRIGGER).forEach { wm.cancelUniqueWork(it) }
    }

    /** Running/enqueued backup work infos (manual + background), for the status screen. */
    fun observe(context: Context): Flow<List<WorkInfo>> {
        val wm = WorkManager.getInstance(context)
        return combine(
            wm.getWorkInfosForUniqueWorkFlow(WORK_MANUAL),
            wm.getWorkInfosForUniqueWorkFlow(WORK_RUN),
            wm.getWorkInfosForUniqueWorkFlow(WORK_PERIODIC),
        ) { a, b, c -> a + b + c }
    }
}

object BackupNotifications {
    const val CHANNEL_PROGRESS = "backup_progress"
    const val CHANNEL_RESULT = "backup_result"
    const val PROGRESS_ID = 4201
    private const val RESULT_ID = 4202

    fun ensureChannels(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val nm = context.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL_PROGRESS, context.getString(R.string.notif_channel_progress), NotificationManager.IMPORTANCE_LOW)
        )
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL_RESULT, context.getString(R.string.notif_channel_result), NotificationManager.IMPORTANCE_DEFAULT)
        )
    }

    private fun contentIntent(context: Context): PendingIntent = PendingIntent.getActivity(
        context, 0,
        Intent(context, MainActivity::class.java).putExtra(MainActivity.EXTRA_OPEN_BACKUP, true)
            .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    fun progressNotification(context: Context, p: BackupProgress, workId: java.util.UUID?): android.app.Notification {
        val b = NotificationCompat.Builder(context, CHANNEL_PROGRESS)
            .setSmallIcon(R.drawable.ic_stat_backup)
            .setContentTitle(context.getString(R.string.notif_backup_running))
            .setContentText(
                if (p.total > 0) context.getString(R.string.notif_backup_progress, p.done, p.total) + (p.currentFile?.let { " · $it" } ?: "")
                else context.getString(R.string.notif_backup_preparing)
            )
            .setProgress(p.total, p.done, p.total == 0)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setContentIntent(contentIntent(context))
        if (workId != null) {
            b.addAction(
                android.R.drawable.ic_menu_close_clear_cancel,
                context.getString(R.string.cancel),
                WorkManager.getInstance(context).createCancelPendingIntent(workId),
            )
        }
        return b.build()
    }

    fun notifyProgress(context: Context, p: BackupProgress) {
        if (!NotificationManagerCompat.from(context).areNotificationsEnabled()) return
        try {
            NotificationManagerCompat.from(context).notify(PROGRESS_ID, progressNotification(context, p, null))
        } catch (_: SecurityException) {
        }
    }

    fun notifyDone(context: Context, r: BackupResult) {
        if (!NotificationManagerCompat.from(context).areNotificationsEnabled()) return
        val n = NotificationCompat.Builder(context, CHANNEL_RESULT)
            .setSmallIcon(R.drawable.ic_stat_backup)
            .setContentTitle(context.getString(R.string.notif_backup_done))
            .setContentText(context.getString(R.string.notif_backup_result, r.uploaded, r.failed))
            .setAutoCancel(true)
            .setContentIntent(contentIntent(context))
            .build()
        try {
            NotificationManagerCompat.from(context).notify(RESULT_ID, n)
        } catch (_: SecurityException) {
        }
    }
}

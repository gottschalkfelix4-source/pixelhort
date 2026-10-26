package app.pixelhort.data.db

import android.content.Context
import androidx.room.Dao
import androidx.room.Database
import androidx.room.Entity
import androidx.room.Index
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.PrimaryKey
import androidx.room.Query
import androidx.room.Room
import androidx.room.RoomDatabase
import androidx.room.Update
import kotlinx.coroutines.flow.Flow

object BackupStatus {
    const val PENDING = "PENDING"
    const val UPLOADED = "UPLOADED"
    const val DUPLICATE = "DUPLICATE"
    const val ERROR = "ERROR"
}

/** One row per local MediaStore item that lives in a selected backup album. */
@Entity(tableName = "backup_item", indices = [Index("bucketId"), Index("status")])
data class BackupItem(
    @PrimaryKey val localId: Long,
    val bucketId: String,
    val bucketName: String,
    val displayName: String,
    val mimeType: String,
    val isVideo: Boolean,
    val size: Long,
    val dateTaken: Long,
    val dateModified: Long,
    val checksum: String? = null,
    /** dateModified/size the checksum was computed for (re-hash if the file changed). */
    val checksumFor: String? = null,
    val status: String = BackupStatus.PENDING,
    val remoteAssetId: String? = null,
    val error: String? = null,
    val attempts: Int = 0,
    val updatedAt: Long = System.currentTimeMillis(),
)

data class StatusCount(val status: String, val count: Int)

@Dao
interface BackupDao {
    @Insert(onConflict = OnConflictStrategy.IGNORE)
    suspend fun insertIgnore(items: List<BackupItem>): List<Long>

    @Update
    suspend fun update(item: BackupItem)

    @Query("SELECT * FROM backup_item WHERE localId IN (:ids)")
    suspend fun byIds(ids: List<Long>): List<BackupItem>

    @Query("SELECT localId FROM backup_item WHERE bucketId = :bucketId")
    suspend fun idsInBucket(bucketId: String): List<Long>

    @Query("UPDATE backup_item SET bucketId = :bucketId, bucketName = :bucketName WHERE localId = :id")
    suspend fun moveToBucket(id: Long, bucketId: String, bucketName: String)

    @Query("DELETE FROM backup_item WHERE localId IN (:ids)")
    suspend fun deleteIds(ids: List<Long>)

    @Query(
        "SELECT * FROM backup_item WHERE bucketId IN (:buckets) AND (status = 'PENDING' OR (status = 'ERROR' AND attempts < :maxAttempts)) " +
            "AND (:videos = 1 OR isVideo = 0) ORDER BY dateTaken DESC"
    )
    suspend fun pending(buckets: List<String>, videos: Boolean, maxAttempts: Int): List<BackupItem>

    @Query(
        "SELECT status, COUNT(*) AS count FROM backup_item WHERE bucketId IN (:buckets) AND (:videos = 1 OR isVideo = 0) GROUP BY status"
    )
    fun statusCounts(buckets: List<String>, videos: Boolean): Flow<List<StatusCount>>

    @Query("SELECT * FROM backup_item WHERE status = 'ERROR' ORDER BY updatedAt DESC LIMIT 200")
    fun errors(): Flow<List<BackupItem>>

    @Query("UPDATE backup_item SET attempts = 0, status = 'PENDING', error = NULL WHERE status = 'ERROR'")
    suspend fun resetErrors()

    @Query("DELETE FROM backup_item")
    suspend fun clear()
}

@Database(entities = [BackupItem::class], version = 1, exportSchema = true)
abstract class BackupDatabase : RoomDatabase() {
    abstract fun backupDao(): BackupDao

    companion object {
        fun create(context: Context): BackupDatabase =
            Room.databaseBuilder(context, BackupDatabase::class.java, "backup.db")
                .fallbackToDestructiveMigration(true)
                .build()
    }
}

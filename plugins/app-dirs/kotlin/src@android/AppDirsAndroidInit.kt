package rs.koffi.appdirs

import android.content.Context

/**
 * Initializes the app-dirs plugin with paths extracted from an Android
 * [Context].
 *
 * Call this **once** from `Application.onCreate()` before any Kotlin code
 * calls [appDirs]:
 *
 * ```kotlin
 * class MyApp : Application() {
 *     override fun onCreate() {
 *         super.onCreate()
 *         AppDirsAndroidInit.init(this)
 *     }
 * }
 * ```
 *
 * Subsequent calls are silently ignored (Rust-side `OnceLock` semantics).
 *
 * ## Why strings instead of a Context reference?
 *
 * Crossing the JNI boundary with a live Java object introduces GC-root
 * bookkeeping and threading constraints. Passing pre-extracted path strings
 * avoids those hazards entirely: the Rust side stores only plain `String`
 * values with no Java object graph attached.
 */
object AppDirsAndroidInit {

    /**
     * Extracts directory paths from [context] and forwards them to the Rust
     * `app_dirs_android_init` function via JNI.
     *
     * @param context An [android.content.Context] - typically the
     *                [android.app.Application] instance.
     */
    @JvmStatic
    fun init(context: Context) {
        appDirsAndroidInit(
            filesDir = context.filesDir.absolutePath,
            cacheDir = context.cacheDir.absolutePath,
            noBackupDir = context.noBackupFilesDir.absolutePath,
            externalFilesDir = context.getExternalFilesDir(null)?.absolutePath ?: "",
        )
    }
}

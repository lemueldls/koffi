package rs.koffi

import kotlin.experimental.ExperimentalNativeApi
import kotlin.native.ref.Cleaner
import kotlin.native.ref.createCleaner

@OptIn(ExperimentalNativeApi::class)
actual abstract class KoffiHandleBase actual constructor(
    actual override val handleId: Long
) : KoffiHandle {

    actual override var isClosed: Boolean = false
        private set

    // Cleaner must not capture 'this'. We pass handleId as the state.
    @Suppress("unused")
    private val cleaner: Cleaner = createCleaner(handleId) { id ->
        KoffiRuntime.releaseHandle?.invoke(id)
    }

    actual override fun close() {
        if (!isClosed) {
            isClosed = true
            KoffiRuntime.releaseHandle?.invoke(handleId)
        }
    }
}

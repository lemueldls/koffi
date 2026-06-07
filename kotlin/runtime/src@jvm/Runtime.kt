package rs.koffi

import java.lang.ref.Cleaner

actual abstract class KoffiHandleBase actual constructor(
    actual override val handleId: Long
) : KoffiHandle {

    private val cleanable: Cleaner.Cleanable

    actual override var isClosed: Boolean = false
        private set

    init {
        cleanable = cleaner.register(this, ReleaseAction(handleId))
    }

    actual override fun close() {
        if (!isClosed) {
            isClosed = true
            cleanable.clean()
        }
    }

    private class ReleaseAction(private val id: Long) : Runnable {
        override fun run() {
            koffi_handle_release(id)
        }
    }

    companion object {
        private val cleaner: Cleaner = Cleaner.create()

        @JvmStatic
        private external fun koffi_handle_release(handleId: Long)
    }
}

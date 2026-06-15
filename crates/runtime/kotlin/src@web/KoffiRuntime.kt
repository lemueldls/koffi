package rs.koffi

import kotlin.js.ExperimentalWasmJsInterop
import kotlin.js.JsAny
import kotlin.js.JsNumber
import kotlin.js.toJsNumber

// External declaration for JS FinalizationRegistry
@OptIn(ExperimentalWasmJsInterop::class)
private external class FinalizationRegistry(cleanupCallback: (JsAny?) -> Unit) : JsAny {
    fun register(target: JsAny, heldValue: JsAny)
}

@OptIn(ExperimentalWasmJsInterop::class)
private fun newFinalizationRegistry(callback: (JsAny?) -> Unit): FinalizationRegistry =
    js("new FinalizationRegistry(callback)")

@OptIn(ExperimentalWasmJsInterop::class)
actual abstract class KoffiHandleBase actual constructor(
    actual override val handleId: Long
) : KoffiHandle {

    actual final override var isClosed: Boolean = false
        private set

    init {
        // Register this handle with the finalizer
        val heldValue = handleId.toDouble().toJsNumber()
        finalizerRegistry?.register(this.asJsAny(), heldValue)
    }

    actual override fun close() {
        if (!isClosed) {
            isClosed = true
            KoffiRuntime.releaseHandle?.invoke(handleId)
        }
    }

    companion object {
        private fun KoffiHandleBase.asJsAny(): JsAny = this.toJsReference()

        private val finalizerRegistry: FinalizationRegistry? by lazy {
            try {
                createFinalizationRegistry { heldValue ->
                    if (heldValue != null) {
                        val num = heldValue.unsafeCast<JsNumber>()
                        val id = num.toDouble().toLong()
                        KoffiRuntime.releaseHandle?.invoke(id)
                    }
                }
            } catch (e: Throwable) {
                null
            }
        }

        private fun createFinalizationRegistry(callback: (JsAny?) -> Unit): FinalizationRegistry {
            return newFinalizationRegistry(callback)
        }
    }
}

package bench

import hello_kotlin.*
import kotlin.time.TimeSource

// One workload per typed shape the glue has to special. Every workload is
// `(Int) -> Long`: run `n` calls, return the accumulated result so the JIT
// can't drop the work. The driver chooses `n` for warmup and for timing
// (native calls are opaque to the optimizer anyway, so no blackhole glue).
const val WARMUP_CALLS = 500_000
const val SAMPLE_CALLS = 1_000_000

fun runBench(args: Array<String>) {
    val scaleDown = if (args.isNotEmpty()) args[0].toInt() else 1
    println("koffi bench: median ns/call, ${SAMPLE_CALLS / scaleDown / 1000}k calls per sample")
    work("scalar", ::scalar, scaleDown)
    work("option wrapper", ::optWrapper, scaleDown)
    work("result wrapper", ::resultWrapper, scaleDown)
    work("struct roundtrip", ::structRoundtrip, scaleDown)
    work("nested struct", ::nestedStruct, scaleDown)
    work("span param", ::spanParam, scaleDown)
    work("span struct roundtrip", ::spanStruct, scaleDown)
    work("string return", ::stringReturn, scaleDown)
    work("bytes return", ::bytesReturn, scaleDown)
    work("opaque handle", ::opaqueHandle, scaleDown)
    println("done")
}

private fun work(name: String, f: (Int) -> Long, scale: Int) {
    val samples = LongArray(5)
    val n = SAMPLE_CALLS / scale
    val warmup = WARMUP_CALLS / scale
    var warm = 0
    while (warm < warmup) {
        f(1)
        warm++
    }
    for (s in samples.indices) {
        val start = TimeSource.Monotonic.markNow()
        val acc = f(n)
        samples[s] = start.elapsedNow().inWholeNanoseconds
        if (acc < 0) throw IllegalStateException("bench accumulator underflow for $name")
    }
    samples.sort()
    val per = kotlin.math.round(samples[2] / n.toDouble() * 10.0) / 10.0
    println("${name.padEnd(22)}: $per ns/op")
}

private fun scalar(n: Int): Long {
    var acc = 0L
    for (i in 0 until n) acc += addOptional(5u, 3u).toLong()
    return acc
}

private fun optWrapper(n: Int): Long {
    var acc = 0L
    for (i in 0 until n) acc += addOptional(5u, null).toLong()
    return acc
}

private fun resultWrapper(n: Int): Long {
    var acc = 0L
    for (i in 0 until n) acc += sumOk(divide(10u, 2u))
    return acc
}

private fun sumOk(r: KoffiResult<UInt, UByte>): Long =
    if (r is KoffiResult.Ok) r.value.toLong() else 0

private fun structRoundtrip(n: Int): Long {
    var acc = 0L
    for (i in 0 until n) acc += rectTopLeft(Rect(Point(0, 0), Point(3, 4))).x
    return acc
}

private fun nestedStruct(n: Int): Long {
    var acc = 0L
    for (i in 0 until n) acc += rectArea(Rect(Point(0, 0), Point(3, 4)))
    return acc
}

private fun spanParam(n: Int): Long {
    var acc = 0L
    for (i in 0 until n) if (stringPair("koffi", "koffi")) acc++
    return acc
}

private fun spanStruct(n: Int): Long {
    var acc = 0L
    for (i in 0 until n) acc += upgrade(Mail(Greeting("Hi", byteArrayOf(9, 9)), 3u)).hops.toLong()
    return acc
}

private fun stringReturn(n: Int): Long {
    var acc = 0L
    for (i in 0 until n) acc += greet(Greeting("Ada", byteArrayOf(1, 2))).length
    return acc
}

private fun bytesReturn(n: Int): Long {
    var acc = 0L
    for (i in 0 until n) acc += echoBytes(byteArrayOf(1, 2, 3)).size
    return acc
}

private fun opaqueHandle(n: Int): Long {
    var acc = 0L
    for (i in 0 until n) acc += describeWindow(Window.open(1uL)).toLong()
    return acc
}
import hello_kotlin.*
import kotlin.system.exitProcess

var failures = 0

fun check(name: String, cond: Boolean) {
    if (cond) {
        println("ok   - $name")
    } else {
        failures++
        println("FAIL - $name")
    }
}

fun main() {
    // Free fns: struct return, enum return, scalars.
    check("hello() -> Payload(data=42)", HelloKotlinFfi.hello().data == 42u.toUShort())

    // Constructors and companion fns on Payload.
    check("Payload.new(7) -> data=7", Payload.new(7u.toUShort()).data == 7u.toUShort())
    check("Payload(5) primary ctor -> data=5", Payload(5u.toUShort()).data == 5u.toUShort())
    check("Payload.invoke(3) -> data=3", Payload(3u.toUShort()).data == 3u.toUShort())

    // Instance methods on Payload.
    val payload = Payload.new(42u.toUShort())
    check("payload.describe() == 42", payload.describe() == 42u)
    check("payload.withData(9) == data=9", payload.withData(9u.toUShort()).data == 9u.toUShort())
    check("Payload.describeFormat() == 1", Payload.describeFormat() == 1u)

    // Free fns: data-carrying enum as arg and return.
    check("HelloKotlinFfi.makeStatus() -> Busy(7)", HelloKotlinFfi.makeStatus() == Status.Busy(7u))
    check("HelloKotlinFfi.statusCode(Busy(7)) == 7", HelloKotlinFfi.statusCode(Status.Busy(7u)) == 7u)
    check("HelloKotlinFfi.statusCode(Error(code=42)) == 42", HelloKotlinFfi.statusCode(Status.Error(code = 42u)) == 42u)
    check("HelloKotlinFfi.statusCode(Idle) == 0", HelloKotlinFfi.statusCode(Status.Idle) == 0u)
    check("HelloKotlinFfi.statusCode(Failed) == 3", HelloKotlinFfi.statusCode(Status.Failed) == 3u)

    // Constructors and companion fns on Status.
    check("Status.idle() -> Idle", Status.idle() == Status.Idle)
    check("Status.newBusy(11) -> Busy(11)", Status.newBusy(11u) == Status.Busy(11u))
    check("Status(2) invoke -> Busy(2)", Status(2u) == Status.Busy(2u))
    check("Status.invoke() -> Idle", Status() == Status.Idle)

    // Instance method on a data-carrying enum with a struct variant.
    val err = Status.Error(code = 42u)
    check("Status.Error(42).describe() == 42", err.describe() == 42u)
    check("Status.Failed.describe() == 2", Status.Failed.describe() == 2u)

    // Fieldless enum with negative discriminant wrapping to UInt.
    check("HelloKotlinFfi.cStatus() -> Err", HelloKotlinFfi.cStatus() == CStatus.Err)
    check("HelloKotlinFfi.cStatusIsErr(Err) == true", HelloKotlinFfi.cStatusIsErr(CStatus.Err))
    check("HelloKotlinFfi.cStatusIsErr(Ok) == false", !HelloKotlinFfi.cStatusIsErr(CStatus.Ok))
    check("CStatus.fromDiscriminant(0u) -> Ok", CStatus.fromDiscriminant(0u) == CStatus.Ok)
    check("CStatus.fromDiscriminant(4294967295u) -> Err", CStatus.fromDiscriminant(4294967295u) == CStatus.Err)
    check("CStatus.Err.discriminant == 4294967295u", CStatus.Err.discriminant == 4294967295u)

    // Struct with an enum field: make + roundtrip through holder.
    val holder = HelloKotlinFfi.makeHolder()
    check("HelloKotlinFfi.makeHolder().status == Error(42)", holder.status == Status.Error(code = 42u))
    check("HelloKotlinFfi.makeHolder().tag == 9", holder.tag == 9u.toUByte())
    check("HelloKotlinFfi.holderStatus(HelloKotlinFfi.makeHolder()) == Error(42)", HelloKotlinFfi.holderStatus(holder) == Status.Error(code = 42u))
    check("HelloKotlinFfi.holderStatus(StatusHolder(Busy(5), 1u)) == Busy(5)", HelloKotlinFfi.holderStatus(StatusHolder(Status.Busy(5u), 1u.toUByte())) == Status.Busy(5u))

    if (failures > 0) {
        println("\n$failures test(s) FAILED")
        exitProcess(1)
    }
    println("\nall tests passed")
}

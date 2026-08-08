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

fun runChecks() {
    // Free fns: struct return, enum return, scalars.
    check("hello() -> Payload(data=42)", hello().data == 42u.toUShort())

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
    check("makeStatus() -> Busy(7)", makeStatus() == Status.Busy(7u))
    check("statusCode(Busy(7)) == 7", statusCode(Status.Busy(7u)) == 7u)
    check("statusCode(Error(code=42)) == 42", statusCode(Status.Error(code = 42u)) == 42u)
    check("statusCode(Idle) == 0", statusCode(Status.Idle) == 0u)
    check("statusCode(Failed) == 3", statusCode(Status.Failed) == 3u)

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
    check("cStatus() -> Err", cStatus() == CStatus.Err)
    check("cStatusIsErr(Err) == true", cStatusIsErr(CStatus.Err))
    check("cStatusIsErr(Ok) == false", !cStatusIsErr(CStatus.Ok))
    check("CStatus.fromDiscriminant(0u) -> Ok", CStatus.fromDiscriminant(0u) == CStatus.Ok)
    check("CStatus.fromDiscriminant(4294967295u) -> Err", CStatus.fromDiscriminant(4294967295u) == CStatus.Err)
    check("CStatus.Err.discriminant == 4294967295u", CStatus.Err.discriminant == 4294967295u)

    // Struct with an enum field: make + roundtrip through holder.
    val holder = makeHolder()
    check("makeHolder().status == Error(42)", holder.status == Status.Error(code = 42u))
    check("makeHolder().tag == 9", holder.tag == 9u.toUByte())
    check("holderStatus(makeHolder()) == Error(42)", holderStatus(holder) == Status.Error(code = 42u))
    check(
        "holderStatus(StatusHolder(Busy(5), 1u)) == Busy(5)",
        holderStatus(StatusHolder(Status.Busy(5u), 1u.toUByte())) == Status.Busy(5u)
    )

    // Struct-typed struct fields: Rect contains two Points.
    val rect = makeRect()
    check("makeRect().topLeft == Point(1,2)", rect.topLeft == Point(1, 2))
    check("makeRect().bottomRight == Point(5,8)", rect.bottomRight == Point(5, 8))
    check("rectArea(makeRect()) == 24", rectArea(rect) == 24)
    check("rectTopLeft(makeRect()) == Point(1,2)", rectTopLeft(rect) == Point(1, 2))
    check("rectArea(Rect(Point(0,0), Point(3,4))) == 12", rectArea(Rect(Point(0, 0), Point(3, 4))) == 12)

    // Option<T> as scalar param and return: None maps to null.
    check("addOptional(5, Some(3)) == 8", addOptional(5u, 3u) == 8u)
    check("addOptional(5, null) == 5", addOptional(5u, null) == 5u)
    check("favorite() == Some(7)", favorite() == 7u)
    check("nothing() == null", nothing() == null)

    // Option of a struct.
    check("maybePayload(true) == Some(data=11)", maybePayload(true)?.data == 11u.toUShort())
    check("maybePayload(false) == null", maybePayload(false) == null)

    // Option<bool> and Option<f64> param marshalling.
    check("paint(Some(true))", paint(true))
    check("paint(null) == false", !paint(null))
    check("howLong(Some(2.5)) == 2.5", howLong(2.5) == 2.5)
    check("howLong(null) == 1.5", howLong(null) == 1.5)

    // Result<T, E> return and param: Ok/Err via the sealed class.
    check("divide(10, 2) is Ok(5)", divide(10u, 2u) == KoffiResult.Ok(5u))
    check("divide(10, 0) is Err(1)", divide(10u, 0u) == KoffiResult.Err(1u.toUByte()))
    check("resultValue(Ok(9)) == 9", resultValue(KoffiResult.Ok(9u)) == 9u)
    check("resultValue(Err(2)) == 0", resultValue(KoffiResult.Err(2u.toUByte())) == 0u)

    // Result<Payload, Status>: data enum on the error side.
    val nom = nominate(50u)
    check("nominate(50) is Ok(Payload(50))", nom is KoffiResult.Ok && nom.value.data == 50u.toUShort())
    val nerr = nominate(500u)
    check("nominate(500) is Err(Busy(500))", nerr is KoffiResult.Err && nerr.error == Status.Busy(500u))

    // Nested wrapper: Result<Option<u32>, u8>.
    check("drink(4) is Ok(Some(4))", drink(4u) == KoffiResult.Ok(4u))
    check("drink(3) is Ok(null)", drink(3u) == KoffiResult.Ok(null))

    // Struct with an Option field, roundtripped through an Option param+return.
    val rode = ride(Dancer(3u, true))
    check("ride(Some(Dancer(3, true))) -> Some(id=4)", rode?.id == 4u)
    check("ride(...).active == Some(true)", rode?.active == true)
    check("ride(null) == null", ride(null) == null)

    // Data enum with an Option variant payload (memory-backed variant branch).
    check("mood() == Flying(Some(true))", mood() == Mood.Flying(true))
    check("moodIsFlying(Flying(Some(true)))", moodIsFlying(Mood.Flying(true)))
    check("moodIsFlying(Flying(null)) == false", !moodIsFlying(Mood.Flying(null)))
    check("moodIsFlying(Fine) == false", !moodIsFlying(Mood.Fine))

    // Opaque handle: Window is a handle class wrapping the native
    // address; the impl block's fns surface as instance methods and
    // companion fns on it.
    val w = Window.open(42uL)
    check("Window.open(42).describe() == 42", w.describe() == 42uL)
    check("Window.retag(w, 99) == 99", w.retag(99uL) == 99uL)
    check("description reflects the retag", w.describe() == 99uL)
    check("describeWindow(w) == 99", describeWindow(w) == 99uL)

    // Opaque nested inside a plain struct: WindowPair carries two handles.
    val pair = WindowPair.new(1uL, 2uL)
    check("WindowPair.new(1,2).tag == 7", pair.tag == 7u.toUByte())
    check("WindowPair.new(1,2).a.describe() == 1", pair.a.describe() == 1uL)
    check("WindowPair.new(1,2).b.describe() == 2", pair.b.describe() == 2uL)
    check("pair.firstDescribe() == 1", pair.firstDescribe() == 1uL)
    check("pair.retagA(w, 42) == 42", pair.retagA(42uL) == 42uL)

    // Proxy field: SafePacket.secret crosses as SecretWire, converted
    // through the user's TryFrom pair on both directions.
    val enc = encrypt(SafePacket(SecretWire(1u, 2u, 3u, 4u), 9u))
    check("encrypt reverses secret bytes", enc.secret == SecretWire(4u, 3u, 2u, 1u))
    check("encrypt bumps hop", enc.hop == 10u.toUByte())

    if (failures > 0) {
        println("\n$failures test(s) FAILED")
        exitProcess(1)
    }
    println("\nall tests passed")
}

import java.lang.foreign.*
import java.lang.invoke.MethodHandle
import rs.koffi.KoffiRuntime

fun main() {
    println("what")

    val result = KoffiFfm.processUser(UserProfile(id = 42u, active = true), factor = 1u)
    println("$result")
}

import rs.koffi.example.addI32

import rs.koffi.example.greetUser
import rs.koffi.example.reverseByteArray
import rs.koffi.example.DatabaseConnection
import rs.koffi.example.maybeFloat
import rs.koffi.example.createProfile
import rs.koffi.example.formatProfile
import rs.koffi.example.UserRole
import rs.koffi.example.roleToString
import rs.koffi.example.roleFromString

fun main() {
    val result = addI32(2, 3)
    println("Result of addI32(2, 3): $result")

    val greeting = greetUser("Alice")
    println(greeting)

    val byteArray = byteArrayOf(1, 2, 3, 4, 5)
    val reversedArray = reverseByteArray(byteArray)
    println("Original byte array: ${byteArray.joinToString(", ")}")
    println("Reversed byte array: ${reversedArray.joinToString(", ")}")

    val dbConnection = DatabaseConnection.open("localhost:5432/mydb")
    println("Database URL: ${dbConnection.getUrl()}")
    println("Is database open? ${dbConnection.isOpen()}")

    val f = maybeFloat()
    println("float: $f")

    val profile = createProfile(123uL, "John Doe", true)
    println("Profile: $profile")

    val format = formatProfile(profile)
    println("Profile format: $format")

    val role = UserRole.Guest(456u)
    val roleString = roleToString(role)
    println("User role: $roleString")
    val roleFromString = roleFromString(roleString)
    println("User role from string: $roleFromString")
}

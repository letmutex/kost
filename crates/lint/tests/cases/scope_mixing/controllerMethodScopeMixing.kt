class UserController {
    fun shortAction() {
        println("short")
    }

    fun longAction() {
        val a = 1
        val b = 2
        val c = 3
        println(a + b + c)
    }
}

class UserHelper {
    fun longHelper() {
        val a = 1
        val b = 2
        val c = 3
        println(a + b + c)
    }
}

fun topLevelLongFunction() {
    val a = 1
    val b = 2
    val c = 3
    println(a + b + c)
}

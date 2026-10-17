val list: List<String> = emptyList()

val map: Map<String, List<Int>> = emptyMap()

class Box<T>(val item: T)

fun <T> identity(x: T): T = x

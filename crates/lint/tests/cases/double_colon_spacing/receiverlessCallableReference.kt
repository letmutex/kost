fun isEven(n: Int) = n % 2 == 0

fun test() {
    val f = ::println
    val list = listOf(1, 2, 3)
    val filtered = list.filter(::isEven)
    filtered.forEach(::println)
}

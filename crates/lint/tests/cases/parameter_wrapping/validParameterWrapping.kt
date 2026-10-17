fun foo(
    bar: String = "default"
)

fun bar(
    @Deprecated("old")
    val x: Int
)

fun single(x: Int = 1)

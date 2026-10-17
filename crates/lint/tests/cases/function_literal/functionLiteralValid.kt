val f1 = { x: Int -> x * 2 }
val f2 = {
    x: Int,
    y: Int
    ->
    x + y
}
val f3 = { -> }
val f4 = {
    x: Int,
    /* comment */ y: Int
    ->
    x + y
}
val f5 = {
    x: Int
    , y: Int
    ->
    x + y
}

fun testWhen(x: Int) = when (x) {
    1 -> { -> "lambda" }
    else -> { -> "other" }
}

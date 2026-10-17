fun validWhen(x: Int) {
    val a = when (x) {
        1 -> "one"
        2 -> "two"
        else -> "other"
    }

    val b = when (x) {
        1 -> { "one" }
        2 -> { "two" }
        else -> { "other" }
    }
}

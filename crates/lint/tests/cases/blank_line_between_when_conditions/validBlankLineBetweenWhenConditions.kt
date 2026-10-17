fun test(x: Int) {
    when (x) {
        1 -> {
            println("one")
        }

        // comment before branch 2
        2 -> println("two")
    }

    when (x) {
        1 -> "a"
        2 -> "b"
        3 -> "c"
    }
}

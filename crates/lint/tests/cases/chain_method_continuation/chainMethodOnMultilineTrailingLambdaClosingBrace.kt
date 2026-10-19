fun test(list: List<Int>) {
    list
        .fold(0) { acc, x ->
            acc + x
        }.toString()
}

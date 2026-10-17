fun foo() {
    bar(
        x /* comment inside argument */ = 1
    )
    bar(1 /* comment before closing */)
    bar(1, /* comment after argument */ 2)
}
fun bar(x: Int, y: Int = 0) {}


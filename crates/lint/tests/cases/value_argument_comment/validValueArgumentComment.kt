fun foo() {
    bar(
        // Separate line comment
        1,
        2,
    )
    baz({
        // comment inside lambda argument is valid
        println("hello")
    })
}
fun bar(x: Int, y: Int) {}
fun baz(block: () -> Unit) {}


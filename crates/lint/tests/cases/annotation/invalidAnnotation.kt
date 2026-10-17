class Foo {
    @Deprecated("message") fun foo() {}
    @First @Second fun bar() {}
    @First @Second("arg") fun baz() {}
    @Deprecated("old") private fun qux() {}
}

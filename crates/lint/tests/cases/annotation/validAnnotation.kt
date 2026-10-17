class Foo(
    @Field1 @Field2 val x: Int
) {
    @Deprecated("message")
    fun foo() {}

    @First
    @Second
    fun bar() {}

    fun baz(@NotNull @Valid input: String) {}
}

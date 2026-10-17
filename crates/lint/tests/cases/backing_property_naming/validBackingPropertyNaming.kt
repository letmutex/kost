class Foo {
    private var _bar: String = ""
    val bar: String get() = _bar

    private val _list = listOf(1)
    fun getList(): List<Int> = _list
}

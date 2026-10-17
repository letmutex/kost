open class Base {
    open fun foo() {}
}

class Derived : Base() {
    override fun foo() {
        super<Base>.foo()
        val args = arrayOf("a", "b")
        println(*args)
    }
}

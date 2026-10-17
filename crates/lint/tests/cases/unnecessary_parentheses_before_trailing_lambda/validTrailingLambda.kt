fun test() {
    list.filter { it > 0 }
    run {
        println("hi")
    }
    compute(1) { it * 2 }
    noLambda()
}

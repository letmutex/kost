fun invalidBracing(x: Int) {
    if (x > 0) {
        println("pos")
    } else println("neg")

    if (x < 0) println("neg") else {
        println("non-neg")
    }

    if (x > 100) {
        println("large")
    } else if (x > 50)
        println("medium")
    else
        println("small")
}

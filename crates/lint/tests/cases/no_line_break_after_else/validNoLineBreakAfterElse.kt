fun testNoLineBreakAfterElse(condition1: Boolean, condition2: Boolean) {
    if (condition1) {
        println(1)
    } else if (condition2) {
        println(2)
    } else {
        println(3)
    }
}

fun testNestedIfInBracedElse(c1: Boolean, c2: Boolean) {
    if (c1) {
        println(1)
    } else {
        if (c2) {
            println(2)
        }
    }
}

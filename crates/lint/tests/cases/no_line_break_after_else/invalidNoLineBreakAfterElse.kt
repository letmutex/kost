fun testInvalidNoLineBreakAfterElse(condition1: Boolean, condition2: Boolean) {
    if (condition1) {
        println(1)
    } else
    if (condition2) {
        println(2)
    }
}

fun testInvalidLineBreakBeforeBrace(condition: Boolean) {
    if (condition) {
        println(1)
    } else
    {
        println(2)
    }
}

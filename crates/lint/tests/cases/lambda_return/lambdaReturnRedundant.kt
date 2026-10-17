val f1 = bar {
    return@bar "value"
}

val f2 = "hello".let {
    return@let "world"
}

val f3 = "foo".let custom@{
    return@custom "item"
}

val f4 = bar {
    return@bar "value";
}

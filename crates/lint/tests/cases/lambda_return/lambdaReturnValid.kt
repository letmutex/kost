val f1 = bar {
    if (ready) return@bar "early"
    "final"
}

val f2 = "foo".let outer@{ foo ->
    "bar".let {
        return@outer "to outer"
    }
}

val f3 = bar {
    return@bar;
}

val f: () -> Unit = {}
val g: (Int, String) -> Boolean = { _, _ -> true }
val h: String.() -> Unit = {}
val wrappedArrow1: (Int, String) ->
    Boolean = { _, _ -> true }
val wrappedArrow2: (
    Int,
    String,
) -> Boolean = { _, _ -> true }

val result = items
    .filter { it > 0 }
    .map { it * 2 }

val withComment = items
    // filter elements
    .filter { it > 0 }
    // map elements
    .map { it * 2 }

val single = items.filter { it > 0 }.map { it * 2 }

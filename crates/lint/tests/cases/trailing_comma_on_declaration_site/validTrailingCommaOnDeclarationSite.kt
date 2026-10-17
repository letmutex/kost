class Person(
    val name: String,
    val age: Int,
)

fun compute(
    a: Int,
    b: Int,
) {}

enum class Status {
    ACTIVE,
    INACTIVE,
}

fun single(a: Int, b: Int) {}

enum class StatusWithMembers {
    ACTIVE,
    INACTIVE,;

    fun isInactive() = this == INACTIVE
}


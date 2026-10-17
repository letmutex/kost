package com.example

enum class SingleLine { A, B, C }

enum class MultiLine {
    A,
    B,
    C,
}

enum class WithMembers {
    FIRST,
    SECOND;

    fun info(): String = "test"
}

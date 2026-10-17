package com.example

fun foo(a: Int, b: Int) {}

fun testValid() {
    foo(1, 2)
    foo(
        1,
        2
    )
    foo( // trailing comment on open paren
        1,
        2
    )
    listOf(1).forEach {
        println(it)
    }
    listOf(1).forEach { // comment after brace
        println(it)
    }
}

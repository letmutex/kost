package com.example

fun foo(a: Int, b: Int, c: Int) {}

fun testValid() {
    foo(1, 2, 3)
    foo(
        1,
        2,
        3
    )
}

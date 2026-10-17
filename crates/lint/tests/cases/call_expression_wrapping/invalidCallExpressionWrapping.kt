package com.example

fun foo(a: Int, b: Int) {}

fun testInvalid() {
    foo(1,
        2)
    listOf(1).forEach { x -> println(x)
    }
}

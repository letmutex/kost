package com.example

fun testValid() {
    val a = listOf(1, 2).filter { it > 0 }.map { it }
    val b = listOf(1)
        .filter { it > 0 }
        .map { it }
        .sorted()
        .first()
}

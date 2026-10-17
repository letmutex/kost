package com.example

fun testInvalid() {
    val b = listOf(1).filter { it > 0 }.map { it }.sorted().first()
}

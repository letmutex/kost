package com.example

fun testValid() {
    val result = listOf(1)
        .filter { it > 0 }
        ?.map { it }
    val fallback = null
        ?: "default"
}

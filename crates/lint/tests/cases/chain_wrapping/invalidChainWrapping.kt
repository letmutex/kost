package com.example

fun testInvalid() {
    val result = listOf(1).
        filter { it > 0 }
    val fallback = null ?:
        "default"
    val withComment = listOf(1). // trailing comment
        filter { it > 0 }
    val elvisWithComment = null ?: /* block comment */
        "default"
}

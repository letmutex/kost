package com.example

context(a: String, b: Int)
fun singleLine() {}

context(
    a: String,
    b: Int
)
fun multiLine() {}

context(
    @Anno a: String,
    b: Int
)
fun multiLineAnnotated() {}

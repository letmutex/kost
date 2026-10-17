package com.example

fun validIf() {
    if (true) {
        println(1)
    } else {
        println(2)
    }

    if (true) {
        println(1)
    } else if (false) {
        println(2)
    }
}

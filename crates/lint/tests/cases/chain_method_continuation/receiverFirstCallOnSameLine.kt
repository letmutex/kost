package com.example

fun testReceiverFirstCall() {
    val registry =
        PropertyTypeRegistry.builder()
            .defaults()
            .type("text")
}

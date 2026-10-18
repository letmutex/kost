package com.example

public actual open class MalformedInputException
actual constructor(message: String) : Super(0) {
    private val _message = message

    override val message: String?
        get() = _message
}

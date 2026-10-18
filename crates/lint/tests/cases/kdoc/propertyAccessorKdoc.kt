package com.example

class SvrValues {
    val pinBackedMasterKey: String?
        /** Returns null if master key is not backed up by a pin. */
        get() {
            return null
        }

    var counter: Int = 0
        /** Returns the current count. */
        get() = field
        /** Sets a new count value. */
        set(value) {
            field = value
        }
}

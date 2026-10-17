class Container {
    @Suppress("ktlint:standard:no-semi")
    fun suppressedMember() {
        enum class Local { A, B; }
    }

    fun unsuppressedMember() {
        enum class Local { C, D; }
    }
}

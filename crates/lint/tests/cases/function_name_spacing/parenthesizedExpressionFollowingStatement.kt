fun test() {
    val cache = call.receiveCache
    (cache[DoubleReceiveCache::class] as DoubleReceiveCache?)?.dispose()
}

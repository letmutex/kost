package com.example

/**
 * A generalized element.
 */
@Serializable(GenericElementSerializer::class)
public interface GenericElement {
    public companion object {
        public val EmptyObject: GenericElement = GenericElementMap()

        public fun GenericElement?.orEmpty(): GenericElement = this ?: EmptyObject
    }

    /** The serializer. */
    public val elementSerializer: KSerializer<*>
}

/**
 * Extension function.
 */
public inline fun <reified T> GenericElement.asA(): T = deserialize()

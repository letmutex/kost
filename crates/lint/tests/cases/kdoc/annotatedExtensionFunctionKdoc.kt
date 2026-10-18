@Deprecated(message = "deprecated")
public fun ASTNode.replaceWith(node: ASTNode) {
    parent?.addChild(node, this)
}

/**
 * Removes ASTNode from its current parent
 */
@Deprecated(message = "deprecated")
public fun ASTNode.remove() {
    parent?.removeChild(this)
}

@Deprecated(message = "deprecated")
public fun ASTNode.findChild() {
}

/**
 * Unannotated extension function with visibility modifier
 */
public fun ASTNode.unannotated() {
    parent?.removeChild(this)
}


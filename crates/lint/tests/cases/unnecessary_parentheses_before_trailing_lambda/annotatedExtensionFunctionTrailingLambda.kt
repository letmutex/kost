@Deprecated(message = "msg")
public fun ASTNode.remove() {
    parent?.removeChild(this)
}

public fun ASTNode.findChild() {
}

fun ASTNode.bareFunction() {
}

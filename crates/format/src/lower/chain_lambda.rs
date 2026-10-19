use tree_sitter::Node;

pub fn count_chain_lambdas(node: &Node<'_>) -> usize {
    let mut count = 0;
    let mut curr = Some(*node);
    while let Some(n) = curr {
        if n.kind() == "call_expression" {
            let mut cursor = n.walk();
            for child in n
                .children(&mut cursor)
                .filter(|c| c.kind() == "call_suffix")
            {
                let mut suffix_cursor = child.walk();
                for grandchild in child.children(&mut suffix_cursor) {
                    if matches!(grandchild.kind(), "lambda_literal" | "annotated_lambda") {
                        count += 1;
                    }
                }
            }
            curr = n.child(0);
        } else if n.kind() == "navigation_expression" {
            curr = n.child(0);
        } else {
            break;
        }
    }
    count
}

pub fn has_multiline_trailing_lambda(node: &Node<'_>) -> bool {
    if node.kind() != "call_expression" {
        return false;
    }
    let mut cursor = node.walk();
    for child in node
        .children(&mut cursor)
        .filter(|c| c.kind() == "call_suffix")
    {
        let mut suffix_cursor = child.walk();
        for grandchild in child.children(&mut suffix_cursor) {
            if matches!(grandchild.kind(), "lambda_literal" | "annotated_lambda")
                && grandchild.start_position().row != grandchild.end_position().row
            {
                return true;
            }
        }
    }
    false
}

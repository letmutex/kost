use tree_sitter::Node;

pub(crate) fn is_pure_lambda_or_labeled(node: &Node<'_>) -> bool {
    if node.kind() == "lambda_literal" {
        return true;
    }
    if node.kind() == "annotated_lambda" {
        let mut c = node.walk();
        return !node.children(&mut c).any(|ch| ch.kind() == "annotation");
    }
    false
}

pub(crate) fn is_scoping_function(node: &Node<'_>) -> bool {
    if is_pure_lambda_or_labeled(node) {
        return true;
    }
    if node.kind() == "call_expression" {
        let mut callee = None;
        let mut has_lambda = false;
        let mut has_val_args = false;
        let mut has_type_args = false;
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if callee.is_none() {
                callee = Some(child);
            } else if child.kind() == "call_suffix" {
                let mut sc = child.walk();
                for g in child.children(&mut sc) {
                    if is_pure_lambda_or_labeled(&g) {
                        has_lambda = true;
                    } else if g.kind() == "value_arguments" {
                        let mut vac = g.walk();
                        if g.children(&mut vac).any(|v| v.kind() == "value_argument") {
                            has_val_args = true;
                        }
                    } else if g.kind() == "type_arguments" {
                        has_type_args = true;
                    }
                }
            }
        }
        if has_lambda
            && !has_val_args
            && !has_type_args
            && let Some(callee_n) = callee
        {
            if callee_n.kind() == "simple_identifier" {
                return true;
            }
            if callee_n.kind() == "navigation_expression"
                && let Some(rcv) = callee_n.child(0)
                && rcv.kind() == "simple_identifier"
            {
                return true;
            }
        }
    }
    false
}

pub(crate) fn get_chain_root<'a>(node: Node<'a>) -> Node<'a> {
    let mut curr = node;
    loop {
        if curr.kind() == "call_expression"
            && let Some(callee) = curr.child(0)
            && callee.kind() == "navigation_expression"
            && let Some(rcv) = callee.child(0)
        {
            curr = rcv;
            continue;
        } else if curr.kind() == "navigation_expression"
            && let Some(rcv) = curr.child(0)
        {
            curr = rcv;
            continue;
        }
        break;
    }
    curr
}

pub(crate) fn is_scoping_chain_or_lambda(node: &Node<'_>) -> bool {
    if is_scoping_function(node) {
        return true;
    }
    let root = get_chain_root(*node);
    root.id() != node.id() && is_scoping_function(&root)
}

use lint::Linter;

#[test]
fn test_deep_binary_expression_stack_safety() {
    let mut code = String::from("fun test() {\n    val a = \"a0\"");
    for i in 1..2500 {
        use std::fmt::Write;
        write!(&mut code, " + \"a{i}\"").unwrap();
    }
    code.push_str("\n}\n");

    let linter = Linter::standard();
    let handler = std::thread::Builder::new()
        .name("test-stack-worker".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            linter.lint_str(&code);
        })
        .unwrap();
    handler
        .join()
        .expect("Worker thread aborted with stack overflow");
}

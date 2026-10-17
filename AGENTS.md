### Rules

- Make test names clear, test behaviors not use cases; don't use vague or confusing names such as `test_issues`, `test_bug_reproduction`, `test_service_get_orders_method_return_type`; prefer names like `test_trailing_lambda_type_infer`
- Run `fmt` and `check` at the end of the job; don't rerun tests after `cargo fmt/check` if only the formats were updated
- Run `clippy` before tests if you've created some large changes
- Don't waste tokens to read `build_lint.rs`; custom lint rules are in `build.rs`
- Don't waste tokens on writing scripts to check file lines if you just want to check violations; cargo check will do its job
- For bug handling, write tests to fail before updating the source, when possible
- Use `concat_str!` or `concat_smol_str!` instead of `format!` for string concatenation when possible
- AST first matching for features and fixes; skipping tree sitter to use string scanning or string matching requires a strong, unavoidable reason
- Converting typed nodes to strings for string matching is directly considered harmful. Requires explicit approval if it's unavoidable
- Avoid hardcoding built-in types as strings, even for `Any` and `Unit`
- Name readable variables, use single-char or two-char or vague variable names only when writing mathematical or algorithmic code
- Don't trust reviewers blindly, especially their suggestions or proposals
- Don't use prefixed or multiline commit messages


### Planning and implementations

Before planning and after implementing new features and bug fixes, go through every part of the plan:

- Is it an anti-pattern?
- Is it simple and optimal?
- Is it readable and maintainable?
- Is it duplicating other similar functions?
- Is it flexible, or is it a convenient hack? or even worse, will it bring more bugs?

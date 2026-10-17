pub mod qualify;
pub mod resolve;

pub use qualify::qualify_type;
pub use resolve::{
    DEFAULT_KOTLIN_PACKAGES, is_declaration_visible, match_qualified_class, resolve_full_type_name,
    resolve_type_by_imports_or_package,
};

pub mod qualify;
pub mod resolve;
pub mod visibility;

pub use qualify::qualify_type;
pub use resolve::{
    DEFAULT_KOTLIN_PACKAGES, match_qualified_class, resolve_full_type_name,
    resolve_type_by_imports_or_package,
};
pub use visibility::{
    is_aliased_import_target, is_container_member_imported, is_declaration_visible,
};

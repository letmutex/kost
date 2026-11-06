pub mod hover;
pub mod references;
pub mod type_inference;

pub use hover::evaluate_hover_test;
pub use references::evaluate_references_test;
pub use type_inference::evaluate_type_inference_test;

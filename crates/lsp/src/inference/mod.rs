pub mod ast_binary;
pub mod ast_call;
pub mod ast_context;
pub mod ast_control;
pub mod ast_eval;
pub mod ast_literal;
pub mod ast_match;
pub mod ast_narrowing;
pub mod call;
pub mod call_parser;
pub mod constructor;
pub mod context;
pub mod delegate;
pub mod destructuring;
pub mod expression;
pub mod function;
pub mod generics;
pub mod infix;
pub mod lambda;
pub mod lambda_ast;
pub mod lambda_call;
pub mod lambda_call_extension;
pub mod lambda_call_solve;
pub mod lambda_return;
pub mod member;
pub mod member_container;
pub mod member_extension;
pub mod member_ranking;
pub mod model;
pub mod overload;
pub mod qualification;
pub mod scope_function;
pub mod scope_lambda;
pub mod scope_stack;
pub mod signature;
pub(crate) mod signature_ast;
pub(crate) mod signature_ast_generics;
pub mod solver;
pub mod solver_unify;
pub mod special_receiver;
pub mod supertypes;
pub mod symbol_receiver;
pub mod trace;
pub mod typealias;
pub mod types;
pub mod unify;
pub mod variable;

pub const MAX_INFERENCE_DEPTH: usize = 16;

pub use call::{
    CallInfo, parse_call_expression, solve_call_generic_return_type,
    solve_call_generic_return_type_traced, unify_types,
};
pub use context::InferenceContext;
pub use delegate::infer_delegated_property_type;
pub use expression::{
    infer_expression_type, infer_node_type, infer_receiver_type, infer_receiver_type_depth,
    qualify_type, resolve_full_type_name,
};
pub use function::infer_function_return_type;
pub use generics::{extract_generic_type_params, solve_generic_return_type};
pub use lambda::{
    find_last_expression_in_lambda, infer_lambda_it_type, infer_lambda_param_type,
    infer_lambda_return_type,
};
pub use lambda_ast::{collect_enclosing_lambdas, lambda_has_explicit_params};
pub use lambda_call::{is_call_with_lambda, resolve_higher_order_call};
pub use member::{
    resolve_member_type, resolve_member_type_depth, resolve_member_type_depth_traced,
    resolve_member_type_kt_depth_traced,
};

pub use model::PrimitiveType;
pub use overload::{OverloadScore, score_overload_candidate, score_overload_candidate_parsed};
pub use scope_function::{
    collect_visible_containers, infer_scope_receiver, resolve_it_keyword, resolve_it_keyword_traced,
};
pub use scope_stack::{ScopeContext, ScopeReceiver, ScopeReceiverKind, build_scope_context};
pub use signature::{
    GenericParamInfo, SignatureInfo, SignatureParam, are_param_types_compatible,
    get_decl_param_types, parse_signature_params,
};
pub use solver::{ConstraintSystem, TypeConstraint};
pub use supertypes::get_generic_supertypes;
pub use trace::{TraceCollector, TraceSpan, Tracer};
pub use types::{
    KotlinType, TypeArg, Variance, parse_type_signature, parse_type_signature_with_params,
};
pub use variable::{infer_variable_type, infer_variable_type_traced};

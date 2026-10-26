use smol_str::SmolStr;

pub use crate::inference::model::PrimitiveType;

/// Generic type argument variance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variance {
    Invariant,
    Out,
    In,
    Star,
}

/// A generic type argument, e.g. `String`, `out Number`, `in Action`, or `*`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeArg {
    pub variance: Variance,
    pub type_ref: KotlinType,
}

impl TypeArg {
    #[inline]
    pub fn invariant(type_ref: KotlinType) -> Self {
        Self {
            variance: Variance::Invariant,
            type_ref,
        }
    }

    #[inline]
    pub fn out(type_ref: KotlinType) -> Self {
        Self {
            variance: Variance::Out,
            type_ref,
        }
    }

    #[inline]
    pub fn in_var(type_ref: KotlinType) -> Self {
        Self {
            variance: Variance::In,
            type_ref,
        }
    }

    pub const STAR: Self = Self {
        variance: Variance::Star,
        type_ref: KotlinType::DynamicOrAny,
    };
}

/// Core algebraic representation of a Kotlin type
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KotlinType {
    Primitive(PrimitiveType),
    Class {
        name: SmolStr,
        args: Vec<TypeArg>,
    },
    Function {
        receiver: Option<Box<KotlinType>>,
        params: Vec<KotlinType>,
        return_type: Box<KotlinType>,
        is_suspend: bool,
    },
    TypeParam(SmolStr),
    Nullable(Box<KotlinType>),
    Intersection(Vec<KotlinType>),
    Nothing,
    DynamicOrAny,
}

impl KotlinType {
    pub const UNIT: Self = Self::Primitive(PrimitiveType::Unit);
    pub const BOOLEAN: Self = Self::Primitive(PrimitiveType::Boolean);
    pub const BYTE: Self = Self::Primitive(PrimitiveType::Byte);
    pub const SHORT: Self = Self::Primitive(PrimitiveType::Short);
    pub const INT: Self = Self::Primitive(PrimitiveType::Int);
    pub const LONG: Self = Self::Primitive(PrimitiveType::Long);
    pub const FLOAT: Self = Self::Primitive(PrimitiveType::Float);
    pub const DOUBLE: Self = Self::Primitive(PrimitiveType::Double);
    pub const CHAR: Self = Self::Primitive(PrimitiveType::Char);
    pub const STRING: Self = Self::Primitive(PrimitiveType::String);
    pub const ANY: Self = Self::Primitive(PrimitiveType::Any);
    pub const NOTHING: Self = Self::Nothing;
    pub const NUMBER: Self = Self::Class {
        name: smol_str::SmolStr::new_static("kotlin.Number"),
        args: Vec::new(),
    };

    #[inline]
    pub const fn primitive(prim: PrimitiveType) -> Self {
        Self::Primitive(prim)
    }

    #[inline]
    pub fn class(name: impl Into<SmolStr>, args: Vec<TypeArg>) -> Self {
        Self::Class {
            name: name.into(),
            args,
        }
    }

    #[inline]
    pub fn simple_class(name: impl Into<SmolStr>) -> Self {
        Self::Class {
            name: name.into(),
            args: Vec::new(),
        }
    }

    #[inline]
    pub fn type_param(name: impl Into<SmolStr>) -> Self {
        Self::TypeParam(name.into())
    }

    #[inline]
    pub fn function(
        receiver: Option<KotlinType>,
        params: Vec<KotlinType>,
        return_type: KotlinType,
        is_suspend: bool,
    ) -> Self {
        Self::Function {
            receiver: receiver.map(Box::new),
            params,
            return_type: Box::new(return_type),
            is_suspend,
        }
    }

    #[inline]
    pub fn is_nullable(&self) -> bool {
        matches!(self, Self::Nullable(_))
    }

    #[inline]
    pub fn make_nullable(self) -> Self {
        if self.is_nullable() {
            self
        } else {
            Self::Nullable(Box::new(self))
        }
    }

    #[inline]
    pub fn with_nullability(self, is_nullable: bool) -> Self {
        if is_nullable {
            self.make_nullable()
        } else {
            self.make_non_null()
        }
    }

    #[inline]
    pub fn nullable(self) -> Self {
        self.make_nullable()
    }

    #[inline]
    pub fn non_null(self) -> Self {
        self.make_non_null()
    }

    #[inline]
    pub fn parse(sig: &str) -> Option<Self> {
        super::parse_type_signature(sig)
    }

    #[inline]
    pub fn make_non_null(self) -> Self {
        match self {
            Self::Nullable(inner) => *inner,
            other => other,
        }
    }

    #[inline]
    pub fn unwrap_nullable_ref(&self) -> &Self {
        match self {
            Self::Nullable(inner) => inner.unwrap_nullable_ref(),
            other => other,
        }
    }

    #[inline]
    pub fn is_primitive(&self) -> bool {
        matches!(self.unwrap_nullable_ref(), Self::Primitive(_))
    }

    #[inline]
    pub fn is_unit(&self) -> bool {
        matches!(self, Self::Primitive(PrimitiveType::Unit))
    }

    #[inline]
    pub fn is_function(&self) -> bool {
        matches!(self.unwrap_nullable_ref(), Self::Function { .. })
    }

    #[inline]
    pub fn is_any(&self) -> bool {
        match self.unwrap_nullable_ref() {
            Self::Primitive(PrimitiveType::Any) | Self::DynamicOrAny => true,
            Self::Class { name, args } if args.is_empty() => {
                let s = name.as_str();
                s == "Any" || s == "kotlin.Any" || s == "java.lang.Object" || s == "Object"
            }
            _ => false,
        }
    }

    /// Extract the base name (without generic arguments) for index lookups
    #[inline]
    pub fn base_name(&self) -> &str {
        match self.unwrap_nullable_ref() {
            Self::Primitive(p) => p.as_str(),
            Self::Class { name, .. } | Self::TypeParam(name) => {
                let s = name.as_str();
                s.split('<').next().unwrap_or(s).trim()
            }
            Self::Function { .. } => "Function",
            Self::Nothing => "Nothing",
            Self::DynamicOrAny => "Any",
            Self::Intersection(_) => "Any",
            Self::Nullable(_) => unreachable!(),
        }
    }

    /// Zero-cost SmolStr key for direct hash map lookups in index `container_to_decls`
    #[inline]
    pub fn to_container_key(&self) -> SmolStr {
        match self.unwrap_nullable_ref() {
            Self::Primitive(p) => SmolStr::new_static(p.as_str()),
            Self::Class { name, .. } => {
                let s = name.as_str();
                let base = s.split('<').next().unwrap_or(s).trim();
                SmolStr::new(base)
            }
            Self::TypeParam(s) => {
                let base = s.split('<').next().unwrap_or(s.as_str()).trim();
                SmolStr::new(base)
            }
            Self::Function { .. } => SmolStr::new_static("Function"),
            Self::Nothing => SmolStr::new_static("Nothing"),
            Self::DynamicOrAny | Self::Intersection(_) => SmolStr::new_static("Any"),
            Self::Nullable(_) => unreachable!(),
        }
    }

    /// Key for companion object container lookup
    #[inline]
    pub fn to_companion_key(&self) -> SmolStr {
        let base = self.base_name();
        common::concat_smol_str!(base, ".Companion")
    }

    #[inline]
    pub fn package_and_simple_name(&self) -> (Option<&str>, &str) {
        let s = self.base_name();
        match s.rsplit_once('.') {
            Some((pkg, name)) => (Some(pkg), name),
            None => (None, s),
        }
    }

    #[inline]
    pub fn type_arguments(&self) -> &[TypeArg] {
        match self.unwrap_nullable_ref() {
            Self::Class { args, .. } => args.as_slice(),
            _ => &[],
        }
    }

    /// Recursively substitute type parameters with their resolved bindings
    pub fn substitute(
        &self,
        bindings: &ahash::AHashMap<smol_str::SmolStr, KotlinType>,
    ) -> KotlinType {
        match self {
            Self::TypeParam(name) => bindings.get(name).cloned().unwrap_or_else(|| self.clone()),
            Self::Class { name, args } => {
                if args.is_empty()
                    && let Some(rep) = bindings.get(name)
                {
                    return rep.clone();
                }
                let new_args = args
                    .iter()
                    .map(|arg| TypeArg {
                        variance: arg.variance,
                        type_ref: arg.type_ref.substitute(bindings),
                    })
                    .collect();
                Self::Class {
                    name: name.clone(),
                    args: new_args,
                }
            }
            Self::Function {
                receiver,
                params,
                return_type,
                is_suspend,
            } => Self::Function {
                receiver: receiver.as_ref().map(|r| Box::new(r.substitute(bindings))),
                params: params.iter().map(|p| p.substitute(bindings)).collect(),
                return_type: Box::new(return_type.substitute(bindings)),
                is_suspend: *is_suspend,
            },
            Self::Nullable(inner) => inner.substitute(bindings).make_nullable(),
            Self::Intersection(types) => {
                Self::Intersection(types.iter().map(|t| t.substitute(bindings)).collect())
            }
            Self::Primitive(_) | Self::Nothing | Self::DynamicOrAny => self.clone(),
        }
    }
}

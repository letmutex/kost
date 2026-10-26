use std::fmt;

use super::model::{KotlinType, PrimitiveType, TypeArg, Variance};

impl fmt::Display for Variance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invariant => Ok(()),
            Self::Out => write!(f, "out "),
            Self::In => write!(f, "in "),
            Self::Star => write!(f, "*"),
        }
    }
}

impl fmt::Display for TypeArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.variance == Variance::Star {
            write!(f, "*")
        } else {
            write!(f, "{}{}", self.variance, self.type_ref)
        }
    }
}

impl fmt::Display for PrimitiveType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl fmt::Display for KotlinType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primitive(p) => write!(f, "{}", p),
            Self::Class { name, args } => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "<")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ">")?;
                }
                Ok(())
            }
            Self::Function {
                receiver,
                params,
                return_type,
                is_suspend,
            } => {
                if *is_suspend {
                    write!(f, "suspend ")?;
                }
                if let Some(rec) = receiver {
                    if rec.is_function() || rec.is_nullable() {
                        write!(f, "({}).", rec)?;
                    } else {
                        write!(f, "{}.", rec)?;
                    }
                }
                write!(f, "(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p)?;
                }
                write!(f, ") -> {}", return_type)
            }
            Self::TypeParam(name) => write!(f, "{}", name),
            Self::Nullable(inner) => {
                if inner.is_function() {
                    write!(f, "({})?", inner)
                } else {
                    write!(f, "{}?", inner)
                }
            }
            Self::Intersection(types) => {
                for (i, t) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, " & ")?;
                    }
                    write!(f, "{}", t)?;
                }
                Ok(())
            }
            Self::Nothing => write!(f, "Nothing"),
            Self::DynamicOrAny => write!(f, "Any"),
        }
    }
}

impl TypeArg {
    pub fn to_short_string(&self) -> String {
        if self.variance == Variance::Star {
            "*".to_string()
        } else {
            common::concat_str!(self.variance.to_string(), self.type_ref.to_short_string())
        }
    }
}

impl KotlinType {
    pub fn to_short_string(&self) -> String {
        match self {
            Self::Primitive(p) => p.as_str().to_string(),
            Self::Class { name, args } => {
                let simple = name
                    .rsplit_once('.')
                    .map(|(_, s)| s)
                    .unwrap_or(name.as_str());
                let base = simple.split('<').next().unwrap_or(simple).trim();
                if args.is_empty() {
                    base.to_string()
                } else {
                    let mut s = String::from(base);
                    s.push('<');
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            s.push_str(", ");
                        }
                        s.push_str(&arg.to_short_string());
                    }
                    s.push('>');
                    s
                }
            }
            Self::Function {
                receiver,
                params,
                return_type,
                is_suspend,
            } => {
                let mut s = String::new();
                if *is_suspend {
                    s.push_str("suspend ");
                }
                if let Some(rec) = receiver {
                    if rec.is_function() || rec.is_nullable() {
                        s.push('(');
                        s.push_str(&rec.to_short_string());
                        s.push_str(").");
                    } else {
                        s.push_str(&rec.to_short_string());
                        s.push('.');
                    }
                }
                s.push('(');
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    s.push_str(&p.to_short_string());
                }
                s.push_str(") -> ");
                s.push_str(&return_type.to_short_string());
                s
            }
            Self::TypeParam(name) => name.to_string(),
            Self::Nullable(inner) => {
                if inner.is_function() {
                    common::concat_str!("(", inner.to_short_string(), ")?")
                } else {
                    common::concat_str!(inner.to_short_string(), "?")
                }
            }
            Self::Intersection(types) => types
                .iter()
                .map(|t| t.to_short_string())
                .collect::<Vec<_>>()
                .join(" & "),
            Self::Nothing => "Nothing".to_string(),
            Self::DynamicOrAny => "Any".to_string(),
        }
    }
}

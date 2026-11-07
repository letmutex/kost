/// Built-in Kotlin primitive and standard library types (1-byte discriminant, zero heap allocation)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PrimitiveType {
    Unit,
    Boolean,
    Byte,
    Short,
    Int,
    Long,
    Float,
    Double,
    Char,
    String,
    Any,
    Nothing,
}

impl PrimitiveType {
    #[inline]
    pub const fn is_numeric(self) -> bool {
        matches!(
            self,
            Self::Byte | Self::Short | Self::Int | Self::Long | Self::Float | Self::Double
        )
    }

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unit => "Unit",
            Self::Boolean => "Boolean",
            Self::Byte => "Byte",
            Self::Short => "Short",
            Self::Int => "Int",
            Self::Long => "Long",
            Self::Float => "Float",
            Self::Double => "Double",
            Self::Char => "Char",
            Self::String => "String",
            Self::Any => "Any",
            Self::Nothing => "Nothing",
        }
    }

    #[inline]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Unit" | "kotlin.Unit" => Some(Self::Unit),
            "Boolean" | "kotlin.Boolean" | "java.lang.Boolean" => Some(Self::Boolean),
            "Byte" | "kotlin.Byte" | "java.lang.Byte" => Some(Self::Byte),
            "Short" | "kotlin.Short" | "java.lang.Short" => Some(Self::Short),
            "Int" | "kotlin.Int" | "java.lang.Integer" | "Integer" => Some(Self::Int),
            "Long" | "kotlin.Long" | "java.lang.Long" => Some(Self::Long),
            "Float" | "kotlin.Float" | "java.lang.Float" => Some(Self::Float),
            "Double" | "kotlin.Double" | "java.lang.Double" => Some(Self::Double),
            "Char" | "kotlin.Char" | "java.lang.Character" | "Character" => Some(Self::Char),
            "String" | "kotlin.String" | "java.lang.String" => Some(Self::String),
            "Any" | "kotlin.Any" | "java.lang.Object" | "Object" => Some(Self::Any),
            "Nothing" | "kotlin.Nothing" | "java.lang.Void" | "Void" => Some(Self::Nothing),
            _ => None,
        }
    }
}

impl std::fmt::Display for PrimitiveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

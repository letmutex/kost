use crate::inference::model::PrimitiveType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardInterface {
    Any,
    Comparable,
    Number,
    CharSequence,
    Serializable,
    Cloneable,
    Constable,
    ConstantDesc,
    Iterable,
    Collection,
    List,
    Set,
    Sequence,
}

impl StandardInterface {
    #[inline]
    pub fn is_standard_package(pkg: &str) -> bool {
        pkg == "kotlin"
            || pkg.starts_with("kotlin.")
            || pkg == "java.lang"
            || pkg.starts_with("java.lang.")
            || pkg == "java.io"
            || pkg == "java.util"
    }

    pub fn from_qualified_name(name: &str) -> Option<Self> {
        if let Some((pkg, short)) = name.rsplit_once('.') {
            if !Self::is_standard_package(pkg) {
                return None;
            }
            Self::from_name(short)
        } else {
            Self::from_name(name)
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let short = name.rsplit('.').next().unwrap_or(name);
        match short {
            "Any" | "Object" => Some(Self::Any),
            "Comparable" => Some(Self::Comparable),
            "Number" => Some(Self::Number),
            "CharSequence" => Some(Self::CharSequence),
            "Serializable" => Some(Self::Serializable),
            "Cloneable" => Some(Self::Cloneable),
            "Constable" => Some(Self::Constable),
            "ConstantDesc" => Some(Self::ConstantDesc),
            "Iterable" => Some(Self::Iterable),
            "Collection" => Some(Self::Collection),
            "List" => Some(Self::List),
            "Set" => Some(Self::Set),
            "Sequence" => Some(Self::Sequence),
            _ => None,
        }
    }

    #[inline]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Any => "Any",
            Self::Comparable => "Comparable",
            Self::Number => "Number",
            Self::CharSequence => "CharSequence",
            Self::Serializable => "Serializable",
            Self::Cloneable => "Cloneable",
            Self::Constable => "Constable",
            Self::ConstantDesc => "ConstantDesc",
            Self::Iterable => "Iterable",
            Self::Collection => "Collection",
            Self::List => "List",
            Self::Set => "Set",
            Self::Sequence => "Sequence",
        }
    }

    #[inline]
    pub fn is_lub_ignored(&self) -> bool {
        matches!(
            self,
            Self::Any
                | Self::Comparable
                | Self::Serializable
                | Self::Cloneable
                | Self::Constable
                | Self::ConstantDesc
        )
    }

    #[inline]
    pub fn is_collection(&self) -> bool {
        matches!(
            self,
            Self::List | Self::Iterable | Self::Collection | Self::Set | Self::Sequence
        )
    }

    pub fn conforms_primitive(&self, prim: PrimitiveType) -> bool {
        match self {
            Self::Any => true,
            Self::Comparable => matches!(
                prim,
                PrimitiveType::Byte
                    | PrimitiveType::Short
                    | PrimitiveType::Int
                    | PrimitiveType::Long
                    | PrimitiveType::Float
                    | PrimitiveType::Double
                    | PrimitiveType::String
                    | PrimitiveType::Char
                    | PrimitiveType::Boolean
            ),
            Self::Number => matches!(
                prim,
                PrimitiveType::Byte
                    | PrimitiveType::Short
                    | PrimitiveType::Int
                    | PrimitiveType::Long
                    | PrimitiveType::Float
                    | PrimitiveType::Double
            ),
            Self::CharSequence => matches!(prim, PrimitiveType::String),
            _ => false,
        }
    }
}

pub fn is_lub_ignored_name(name: &str) -> bool {
    StandardInterface::from_qualified_name(name).is_some_and(|iface| iface.is_lub_ignored())
}

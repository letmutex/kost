//! Statically known metric property keys across lexical scopes.

use crate::types::Scope;

/// Statically known metric properties across scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyKey {
    // Function scope
    FunctionName,
    FunctionNameLength,
    FunctionLines,
    FunctionBodyLines,
    FunctionParametersCount,
    FunctionAnnotations,
    FunctionVisibility,
    FunctionMaxNesting,
    FunctionCyclomaticComplexity,
    FunctionHasExpressionBody,
    FunctionReturnType,

    // Class scope
    ClassName,
    ClassNameLength,
    ClassLines,
    ClassMethodsCount,
    ClassPropertiesCount,
    ClassAnnotations,
    ClassSupertypes,
    ClassVisibility,
    ClassKind,
    ClassConstructorParametersCount,

    // Variable scope
    VariableName,
    VariableNameLength,
    VariableLines,
    VariableType,
    VariableHasCustomGetter,
    VariableHasCustomSetter,

    // Import scope
    ImportPath,
    ImportAlias,
    ImportIsWildcard,

    // File scope
    FileLines,
    FileFunctionsCount,
    FileImportsCount,
    FileTopLevelDeclarationsCount,
    FileMaxIndentSpaces,
    FilePackage,

    // Lambda scope
    LambdaLines,
}

impl PropertyKey {
    pub fn scope(&self) -> Scope {
        match self {
            Self::FunctionName
            | Self::FunctionNameLength
            | Self::FunctionLines
            | Self::FunctionBodyLines
            | Self::FunctionParametersCount
            | Self::FunctionAnnotations
            | Self::FunctionVisibility
            | Self::FunctionMaxNesting
            | Self::FunctionCyclomaticComplexity
            | Self::FunctionHasExpressionBody
            | Self::FunctionReturnType => Scope::Function,

            Self::ClassName
            | Self::ClassNameLength
            | Self::ClassLines
            | Self::ClassMethodsCount
            | Self::ClassPropertiesCount
            | Self::ClassAnnotations
            | Self::ClassSupertypes
            | Self::ClassVisibility
            | Self::ClassKind
            | Self::ClassConstructorParametersCount => Scope::Class,

            Self::VariableName
            | Self::VariableNameLength
            | Self::VariableLines
            | Self::VariableType
            | Self::VariableHasCustomGetter
            | Self::VariableHasCustomSetter => Scope::Variable,

            Self::ImportPath | Self::ImportAlias | Self::ImportIsWildcard => Scope::Import,

            Self::FileLines
            | Self::FileFunctionsCount
            | Self::FileImportsCount
            | Self::FileTopLevelDeclarationsCount
            | Self::FileMaxIndentSpaces
            | Self::FilePackage => Scope::File,

            Self::LambdaLines => Scope::Lambda,
        }
    }

    pub const fn is_deferred_file_aggregate(&self) -> bool {
        matches!(
            self,
            Self::FileFunctionsCount
                | Self::FileMaxIndentSpaces
                | Self::FileImportsCount
                | Self::FileTopLevelDeclarationsCount
        )
    }

    pub fn from_scope_and_field(scope: Scope, field: &str) -> Option<Self> {
        match (scope, field) {
            (Scope::Function, "name") => Some(Self::FunctionName),
            (Scope::Function, "name.length") => Some(Self::FunctionNameLength),
            (Scope::Function, "lines") => Some(Self::FunctionLines),
            (Scope::Function, "body_lines") => Some(Self::FunctionBodyLines),
            (Scope::Function, "parameters_count") => Some(Self::FunctionParametersCount),
            (Scope::Function, "annotations") => Some(Self::FunctionAnnotations),
            (Scope::Function, "visibility") => Some(Self::FunctionVisibility),
            (Scope::Function, "max_nesting") => Some(Self::FunctionMaxNesting),
            (Scope::Function, "cyclomatic_complexity") => Some(Self::FunctionCyclomaticComplexity),
            (Scope::Function, "has_expression_body") => Some(Self::FunctionHasExpressionBody),
            (Scope::Function, "return_type") => Some(Self::FunctionReturnType),

            (Scope::Class, "name") => Some(Self::ClassName),
            (Scope::Class, "name.length") => Some(Self::ClassNameLength),
            (Scope::Class, "lines") => Some(Self::ClassLines),
            (Scope::Class, "methods_count" | "functions_count") => Some(Self::ClassMethodsCount),
            (Scope::Class, "properties_count") => Some(Self::ClassPropertiesCount),
            (Scope::Class, "annotations") => Some(Self::ClassAnnotations),
            (Scope::Class, "supertypes") => Some(Self::ClassSupertypes),
            (Scope::Class, "visibility") => Some(Self::ClassVisibility),
            (Scope::Class, "kind") => Some(Self::ClassKind),
            (Scope::Class, "constructor_parameters_count") => {
                Some(Self::ClassConstructorParametersCount)
            }

            (Scope::Variable, "name") => Some(Self::VariableName),
            (Scope::Variable, "name.length") => Some(Self::VariableNameLength),
            (Scope::Variable, "lines") => Some(Self::VariableLines),
            (Scope::Variable, "type") => Some(Self::VariableType),
            (Scope::Variable, "has_custom_getter") => Some(Self::VariableHasCustomGetter),
            (Scope::Variable, "has_custom_setter") => Some(Self::VariableHasCustomSetter),

            (Scope::Import, "path") => Some(Self::ImportPath),
            (Scope::Import, "alias") => Some(Self::ImportAlias),
            (Scope::Import, "is_wildcard") => Some(Self::ImportIsWildcard),

            (Scope::File, "lines") => Some(Self::FileLines),
            (Scope::File, "functions_count") => Some(Self::FileFunctionsCount),
            (Scope::File, "imports_count") => Some(Self::FileImportsCount),
            (Scope::File, "top_level_declarations_count") => {
                Some(Self::FileTopLevelDeclarationsCount)
            }
            (Scope::File, "max_indent_spaces") => Some(Self::FileMaxIndentSpaces),
            (Scope::File, "package") => Some(Self::FilePackage),

            (Scope::Lambda, "lines") => Some(Self::LambdaLines),
            _ => None,
        }
    }

    pub fn field_name(&self) -> &'static str {
        match self {
            Self::FunctionName | Self::ClassName | Self::VariableName => "name",
            Self::FunctionNameLength | Self::ClassNameLength | Self::VariableNameLength => {
                "name.length"
            }
            Self::FunctionLines
            | Self::ClassLines
            | Self::VariableLines
            | Self::FileLines
            | Self::LambdaLines => "lines",
            Self::FunctionBodyLines => "body_lines",
            Self::FunctionParametersCount => "parameters_count",
            Self::FileFunctionsCount => "functions_count",
            Self::FunctionAnnotations | Self::ClassAnnotations => "annotations",
            Self::FunctionVisibility | Self::ClassVisibility => "visibility",
            Self::FunctionMaxNesting => "max_nesting",
            Self::FunctionCyclomaticComplexity => "cyclomatic_complexity",
            Self::FunctionHasExpressionBody => "has_expression_body",
            Self::FunctionReturnType => "return_type",
            Self::ClassMethodsCount => "methods_count",
            Self::ClassPropertiesCount => "properties_count",
            Self::ClassSupertypes => "supertypes",
            Self::ClassKind => "kind",
            Self::ClassConstructorParametersCount => "constructor_parameters_count",
            Self::VariableType => "type",
            Self::VariableHasCustomGetter => "has_custom_getter",
            Self::VariableHasCustomSetter => "has_custom_setter",
            Self::ImportPath => "path",
            Self::ImportAlias => "alias",
            Self::ImportIsWildcard => "is_wildcard",
            Self::FileImportsCount => "imports_count",
            Self::FileTopLevelDeclarationsCount => "top_level_declarations_count",
            Self::FileMaxIndentSpaces => "max_indent_spaces",
            Self::FilePackage => "package",
        }
    }

    pub fn expected_type_name(&self) -> &'static str {
        match self {
            Self::FunctionName
            | Self::FunctionVisibility
            | Self::FunctionReturnType
            | Self::ClassName
            | Self::ClassVisibility
            | Self::ClassKind
            | Self::VariableName
            | Self::VariableType
            | Self::ImportPath
            | Self::ImportAlias
            | Self::FilePackage => "Str",

            Self::FunctionNameLength
            | Self::ClassNameLength
            | Self::VariableNameLength
            | Self::FunctionLines
            | Self::FunctionBodyLines
            | Self::FunctionParametersCount
            | Self::FunctionMaxNesting
            | Self::FunctionCyclomaticComplexity
            | Self::ClassLines
            | Self::ClassMethodsCount
            | Self::ClassPropertiesCount
            | Self::ClassConstructorParametersCount
            | Self::VariableLines
            | Self::FileLines
            | Self::FileFunctionsCount
            | Self::FileImportsCount
            | Self::FileTopLevelDeclarationsCount
            | Self::FileMaxIndentSpaces
            | Self::LambdaLines => "Int",

            Self::FunctionHasExpressionBody
            | Self::VariableHasCustomGetter
            | Self::VariableHasCustomSetter
            | Self::ImportIsWildcard => "Bool",

            Self::FunctionAnnotations | Self::ClassAnnotations | Self::ClassSupertypes => "StrList",
        }
    }
}

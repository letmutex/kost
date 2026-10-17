use smol_str::SmolStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawGenericParamInfo {
    pub name: SmolStr,
    pub bounds: Vec<SmolStr>,
    pub is_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSignatureParam {
    pub name: SmolStr,
    pub type_name: SmolStr,
    pub has_default: bool,
    pub is_vararg: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawSignatureInfo {
    pub type_params: Vec<RawGenericParamInfo>,
    pub receiver: Option<SmolStr>,
    pub params: Vec<RawSignatureParam>,
    pub return_type: Option<SmolStr>,
}

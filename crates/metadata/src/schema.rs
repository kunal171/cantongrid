//! Versioned offline metadata preserves declarations; it never supplies grants or SQL instructions.
use serde::Serialize;

/// Content-addressed DAR signature surface. Package order is lexical; declaration order is retained.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Metadata {
    pub format_version: u32,
    pub artifact_sha256: String,
    pub main_package_id: String,
    pub packages: Vec<Package>,
}

/// One original DALF payload, identified by its verified SHA-256 and explicit LF version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Package {
    pub package_id: String,
    pub name: String,
    pub version: String,
    pub lf_version: String,
    pub upgraded_package_id: Option<String>,
    /// External packages referenced by extracted types; not a bytecode dependency validation claim.
    pub type_dependencies: Vec<String>,
    pub data_types: Vec<DataType>,
    pub synonyms: Vec<Synonym>,
    pub templates: Vec<Template>,
    pub interfaces: Vec<Interface>,
}

/// Qualified LF type identity avoids confusing same-named types in different package versions.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct TypeId {
    pub package_id: String,
    pub module: String,
    pub name: String,
}

/// Type applications retain exact arguments and explicit references, including recursive records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Type {
    Builtin {
        name: String,
        arguments: Vec<Type>,
    },
    Reference {
        target: TypeId,
        arguments: Vec<Type>,
    },
    Synonym {
        target: TypeId,
        arguments: Vec<Type>,
    },
    Variable {
        name: String,
        arguments: Vec<Type>,
    },
    /// Numeric scale parameters remain integers, e.g. Numeric 10 is a builtin with Nat(10).
    Nat {
        value: u8,
    },
    Struct {
        fields: Vec<Field>,
    },
    Forall {
        parameters: Vec<Parameter>,
        body: Box<Type>,
    },
}

/// Ordered record fields and variant constructors share a named type representation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Field {
    pub name: String,
    pub value_type: Type,
}

/// Type parameters retain kinds so generic Numeric scales are not mistaken for data values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Parameter {
    pub name: String,
    pub kind: Kind,
}

/// LF kind applications are explicit rather than inferred from source-language names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    Star,
    Nat,
    Arrow {
        parameters: Vec<Kind>,
        result: Box<Kind>,
    },
}

/// A named declaration retains serializability and formal parameters without expanding recursion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DataType {
    pub id: TypeId,
    pub serializable: bool,
    pub parameters: Vec<Parameter>,
    pub definition: Definition,
}

/// Preserve record, variant, enum and interface identity even when an SQL model cannot project it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Definition {
    Record { fields: Vec<Field> },
    Variant { constructors: Vec<Field> },
    Enum { constructors: Vec<String> },
    Interface,
}

/// Named aliases stay distinguishable from nominal data constructors for later schema comparison.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Synonym {
    pub id: TypeId,
    pub parameters: Vec<Parameter>,
    pub value_type: Type,
}

/// Templates reference their argument record; signatory/controller expressions are not interpreted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Template {
    pub id: TypeId,
    pub key_type: Option<Type>,
    pub choices: Vec<Choice>,
    pub implements: Vec<TypeId>,
}

/// Choice metadata describes an operation, not permission to exercise it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Choice {
    pub name: String,
    pub consuming: bool,
    pub argument_type: Type,
    pub return_type: Type,
}

/// Interface signatures are retained; interface ingestion/authorization is outside this extraction slice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Interface {
    pub id: TypeId,
    pub view_type: Type,
    pub methods: Vec<Field>,
    pub choices: Vec<Choice>,
    pub requires: Vec<TypeId>,
}

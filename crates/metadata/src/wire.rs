//! Metadata-only projection of Digital Asset's Apache-2.0 LF archive schemas.
//! Copyright 2026 Digital Asset (Switzerland) GmbH and/or its affiliates.
//! Source: nodejumper-org/canton-rust-sdk, commit 5caaebc3eab380de827196663d4bb18b90ffd6f4,
//! crates/canton-lf/proto/com/digitalasset/daml/lf/archive/{daml_lf,daml_lf2}.proto.
//! SHA-256: f9375b5d55701ff8953a81257b8e4d54201969c774b923196fb8afce3fa9e24f /
//! 7eddfc7f4eed57acc9ebc8feabbca16070e4e18f66bed1d3a43bb6cf6b16d366.
//! Field tags/types follow those schemas; prost handles the wire encoding. Executable
//! expressions, locations and value definitions are intentionally not decoded. LF
//! version gating occurs before package decoding; this is not an LF bytecode validator.

/// Canonical empty protobuf value.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Unit {}

/// Original payload bytes and their declared package identity.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Archive {
    #[prost(int32, tag = "1")]
    pub hash_function: i32,
    #[prost(bytes = "vec", tag = "3")]
    pub payload: Vec<u8>,
    #[prost(string, tag = "4")]
    pub hash: String,
}

/// Versioned wrapper; both major versions remain distinguishable.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Payload {
    #[prost(string, tag = "3")]
    pub minor: String,
    #[prost(int32, tag = "5")]
    pub patch: i32,
    #[prost(oneof = "payload::Sum", tags = "2, 4")]
    pub sum: Option<payload::Sum>,
}

/// Declarations plus interning tables; expression/value tables are outside metadata.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Package {
    #[prost(message, repeated, tag = "1")]
    pub modules: Vec<Module>,
    #[prost(string, repeated, tag = "2")]
    pub strings: Vec<String>,
    #[prost(message, repeated, tag = "3")]
    pub dotted_names: Vec<DottedName>,
    #[prost(message, optional, tag = "4")]
    pub metadata: Option<PackageMetadata>,
    #[prost(message, repeated, tag = "5")]
    pub types: Vec<Type>,
}

/// A dotted identifier retains ordered interned segments.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct DottedName {
    #[prost(int32, repeated, tag = "1")]
    pub segments: Vec<i32>,
}

/// Package names/versions are metadata, not deployment assertions.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct PackageMetadata {
    #[prost(int32, tag = "1")]
    pub name: i32,
    #[prost(int32, tag = "2")]
    pub version: i32,
    #[prost(message, optional, tag = "3")]
    pub upgraded: Option<Upgraded>,
}

/// Optional original package identity from upgrade metadata.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Upgraded {
    #[prost(int32, tag = "1")]
    pub package_id: i32,
}

/// Only the signature surface is decoded.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Module {
    #[prost(int32, tag = "1")]
    pub name: i32,
    #[prost(message, repeated, tag = "3")]
    pub synonyms: Vec<Synonym>,
    #[prost(message, repeated, tag = "4")]
    pub data_types: Vec<DataType>,
    #[prost(message, repeated, tag = "6")]
    pub templates: Vec<Template>,
    #[prost(message, repeated, tag = "8")]
    pub interfaces: Vec<Interface>,
}

/// LF 2.1 package references are self or an interned package-id string.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct PackageRef {
    #[prost(oneof = "package_ref::Sum", tags = "1, 3")]
    pub sum: Option<package_ref::Sum>,
}

/// A module reference is always qualified by package identity.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ModuleRef {
    #[prost(message, optional, tag = "1")]
    pub package: Option<PackageRef>,
    #[prost(int32, tag = "2")]
    pub name: i32,
}

/// Nominal and synonym references share the same qualified identifier layout.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct TypeRef {
    #[prost(message, optional, tag = "1")]
    pub module: Option<ModuleRef>,
    #[prost(int32, tag = "2")]
    pub name: i32,
}

/// Ordered named record field or variant constructor.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Field {
    #[prost(int32, tag = "3")]
    pub name: i32,
    #[prost(message, optional, tag = "2")]
    pub value_type: Option<Type>,
}

/// Type variable name and its declared kind.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Parameter {
    #[prost(int32, tag = "3")]
    pub name: i32,
    #[prost(message, optional, tag = "2")]
    pub kind: Option<Kind>,
}

/// Kind alternatives known in the supported LF version.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Kind {
    #[prost(oneof = "kind::Sum", tags = "1, 2, 3")]
    pub sum: Option<kind::Sum>,
}

/// Kind arrows retain argument ordering.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct KindArrow {
    #[prost(message, repeated, tag = "1")]
    pub parameters: Vec<Kind>,
    #[prost(message, optional, boxed, tag = "2")]
    pub result: Option<Box<Kind>>,
}

/// Metadata type alternatives; unsupported alternatives cannot become a guessed scalar.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Type {
    #[prost(oneof = "type_node::Sum", tags = "1, 2, 3, 4, 5, 6, 7, 8")]
    pub sum: Option<type_node::Sum>,
}

/// A possibly applied type variable.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Variable {
    #[prost(int32, tag = "3")]
    pub name: i32,
    #[prost(message, repeated, tag = "2")]
    pub arguments: Vec<Type>,
}

/// A possibly applied nominal/synonym type.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Named {
    #[prost(message, optional, tag = "1")]
    pub target: Option<TypeRef>,
    #[prost(message, repeated, tag = "2")]
    pub arguments: Vec<Type>,
}

/// Builtin tag and exact type arguments, including Numeric scale naturals.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Builtin {
    #[prost(int32, tag = "1")]
    pub name: i32,
    #[prost(message, repeated, tag = "2")]
    pub arguments: Vec<Type>,
}

/// Polymorphic type with explicit binders.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Forall {
    #[prost(message, repeated, tag = "1")]
    pub parameters: Vec<Parameter>,
    #[prost(message, optional, boxed, tag = "2")]
    pub body: Option<Box<Type>>,
}

/// Record/variant fields preserve their declaration order.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Fields {
    #[prost(message, repeated, tag = "1")]
    pub fields: Vec<Field>,
}

/// Enum constructors reference the package string table.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Enum {
    #[prost(int32, repeated, tag = "2")]
    pub constructors: Vec<i32>,
}

/// Nominal declaration; interface markers are retained, not flattened.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct DataType {
    #[prost(int32, tag = "2")]
    pub name: i32,
    #[prost(message, repeated, tag = "3")]
    pub parameters: Vec<Parameter>,
    #[prost(bool, tag = "4")]
    pub serializable: bool,
    #[prost(oneof = "data_type::Definition", tags = "5, 6, 7, 8")]
    pub definition: Option<data_type::Definition>,
}

/// A declared type alias.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Synonym {
    #[prost(int32, tag = "2")]
    pub name: i32,
    #[prost(message, repeated, tag = "3")]
    pub parameters: Vec<Parameter>,
    #[prost(message, optional, tag = "4")]
    pub value_type: Option<Type>,
}

/// Template argument identity, choice signatures, key and implemented interfaces.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Template {
    #[prost(int32, tag = "1")]
    pub name: i32,
    #[prost(message, repeated, tag = "6")]
    pub choices: Vec<Choice>,
    #[prost(message, optional, tag = "9")]
    pub key: Option<Key>,
    #[prost(message, repeated, tag = "10")]
    pub implements: Vec<Implements>,
}

/// Only key type is metadata; maintainers/key expressions are not interpreted.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Key {
    #[prost(message, optional, tag = "1")]
    pub value_type: Option<Type>,
}

/// Reference to an implemented interface; its executable body is omitted.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Implements {
    #[prost(message, optional, tag = "1")]
    pub interface: Option<TypeRef>,
}

/// Choice argument/result signatures do not authenticate controllers.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Choice {
    #[prost(int32, tag = "2")]
    pub name: i32,
    #[prost(bool, tag = "3")]
    pub consuming: bool,
    #[prost(message, optional, tag = "6")]
    pub argument: Option<Argument>,
    #[prost(message, optional, tag = "8")]
    pub result: Option<Type>,
}

/// Choice binder type; the local expression variable name is irrelevant to its signature.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Argument {
    #[prost(message, optional, tag = "2")]
    pub value_type: Option<Type>,
}

/// Interface view, method and choice types.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Interface {
    #[prost(int32, tag = "2")]
    pub name: i32,
    #[prost(message, repeated, tag = "3")]
    pub methods: Vec<Method>,
    #[prost(message, repeated, tag = "5")]
    pub choices: Vec<Choice>,
    #[prost(message, optional, tag = "6")]
    pub view: Option<Type>,
    #[prost(message, repeated, tag = "7")]
    pub requires: Vec<TypeRef>,
}

/// Named interface method signature.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Method {
    #[prost(int32, tag = "2")]
    pub name: i32,
    #[prost(message, optional, tag = "3")]
    pub value_type: Option<Type>,
}

/// Explicit known alternatives for payload metadata.
pub(crate) mod payload {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Sum {
        #[prost(bytes, tag = "2")]
        Lf1(Vec<u8>),
        #[prost(bytes, tag = "4")]
        Lf2(Vec<u8>),
    }
}
/// Explicit known alternatives for package ref metadata.
pub(crate) mod package_ref {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Sum {
        #[prost(message, tag = "1")]
        SelfPackage(super::Unit),
        #[prost(int32, tag = "3")]
        Imported(i32),
    }
}
/// Explicit known alternatives for kind metadata.
pub(crate) mod kind {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Sum {
        #[prost(message, tag = "1")]
        Star(super::Unit),
        #[prost(message, tag = "2")]
        Arrow(super::KindArrow),
        #[prost(message, tag = "3")]
        Nat(super::Unit),
    }
}
/// Explicit known alternatives for type node metadata.
pub(crate) mod type_node {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Sum {
        #[prost(message, tag = "1")]
        Variable(super::Variable),
        #[prost(message, tag = "2")]
        Reference(super::Named),
        #[prost(message, tag = "3")]
        Builtin(super::Builtin),
        #[prost(message, tag = "4")]
        Forall(super::Forall),
        #[prost(message, tag = "5")]
        Struct(super::Fields),
        #[prost(sint64, tag = "6")]
        Nat(i64),
        #[prost(message, tag = "7")]
        Synonym(super::Named),
        #[prost(int32, tag = "8")]
        Interned(i32),
    }
}
/// Explicit known alternatives for data type metadata.
pub(crate) mod data_type {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Definition {
        #[prost(message, tag = "5")]
        Record(super::Fields),
        #[prost(message, tag = "6")]
        Variant(super::Fields),
        #[prost(message, tag = "7")]
        Enum(super::Enum),
        #[prost(message, tag = "8")]
        Interface(super::Unit),
    }
}

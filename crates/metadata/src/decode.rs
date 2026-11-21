//! Interpret only verified LF 2.1 signature metadata; expressions remain opaque skipped fields.
use crate::{error::MetadataError as Error, schema as s, wire as w};
use prost::Message;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const MAX_NODES: usize = 250_000;
const MAX_DEPTH: usize = 64;

/// A shared budget bounds expansion across all packages, including repeatedly interned type trees.
pub(crate) struct Budget {
    nodes: usize,
    bytes: usize,
}
impl Budget {
    pub fn new() -> Self {
        Self {
            nodes: MAX_NODES,
            bytes: 64 * 1024 * 1024,
        }
    }
    fn consume(&mut self, bytes: usize) -> Result<(), Error> {
        self.bytes = self.bytes.checked_sub(bytes).ok_or(Error::Limit)?;
        Ok(())
    }
}

/// Verified decoded package plus its exact LF wrapper version.
pub(crate) struct Decoded {
    pub id: String,
    pub package: w::Package,
}

/// Hash original payload bytes before version/type interpretation. Hashing a re-encoded AST is invalid.
pub(crate) fn decode(bytes: &[u8]) -> Result<Decoded, Error> {
    let archive = w::Archive::decode(bytes).map_err(|_| Error::Protobuf)?;
    let computed = hex(&Sha256::digest(&archive.payload));
    if archive.hash_function != 0 || archive.hash != computed {
        return Err(Error::Hash);
    }
    let payload = w::Payload::decode(archive.payload.as_slice()).map_err(|_| Error::Protobuf)?;
    if payload.minor != "1" || payload.patch != 0 {
        return Err(Error::UnsupportedVersion);
    }
    let Some(w::payload::Sum::Lf2(bytes)) = payload.sum else {
        return Err(Error::UnsupportedVersion);
    };
    let package = w::Package::decode(bytes.as_slice()).map_err(|_| Error::Protobuf)?;
    Ok(Decoded {
        id: computed,
        package,
    })
}

/// Stable lower-case hex for package, artifact and metadata identities.
pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

/// A signature traversal has independent node/depth budgets and records only type dependencies.
struct Resolver<'a> {
    id: &'a str,
    p: &'a w::Package,
    budget: &'a mut Budget,
    dependencies: BTreeSet<String>,
}
impl<'a> Resolver<'a> {
    /// One unit of interpretation work, including interning expansion, is charged before recursion.
    fn charge(&mut self, depth: usize) -> Result<(), Error> {
        if depth > MAX_DEPTH {
            return Err(Error::Limit);
        }
        self.budget.nodes = self.budget.nodes.checked_sub(1).ok_or(Error::Limit)?;
        self.budget.consume(128)
    }
    /// Resolve a bounded declaration string, never an arbitrary executable literal.
    fn string(&mut self, index: i32) -> Result<String, Error> {
        let p = self.p;
        let value = p
            .strings
            .get(usize::try_from(index).map_err(|_| Error::Reference)?)
            .ok_or(Error::Reference)?;
        if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
            return Err(Error::Definition);
        }
        self.budget.consume(value.len().saturating_mul(6))?;
        Ok(value.clone())
    }
    /// Dotted-name segments must not themselves contain dots, preventing ambiguous qualification.
    fn name(&mut self, index: i32) -> Result<String, Error> {
        let value = self
            .p
            .dotted_names
            .get(usize::try_from(index).map_err(|_| Error::Reference)?)
            .ok_or(Error::Reference)?;
        if value.segments.is_empty() || value.segments.len() > 128 {
            return Err(Error::Definition);
        }
        let mut names = Vec::new();
        for index in &value.segments {
            let name = self.string(*index)?;
            if name.contains('.') {
                return Err(Error::Definition);
            }
            names.push(name);
        }
        let name = names.join(".");
        if name.len() > 4096 {
            return Err(Error::Limit);
        }
        Ok(name)
    }
    /// References retain package hashes and type arguments instead of recursively expanding records.
    fn reference(&mut self, reference: &w::TypeRef) -> Result<s::TypeId, Error> {
        let module = reference.module.as_ref().ok_or(Error::Reference)?;
        let package = match module.package.as_ref().and_then(|p| p.sum.as_ref()) {
            Some(w::package_ref::Sum::SelfPackage(_)) => self.id.to_owned(),
            Some(w::package_ref::Sum::Imported(index)) => self.string(*index)?,
            None => return Err(Error::Reference),
        };
        if !package_id(&package) {
            return Err(Error::Reference);
        }
        if package != self.id {
            self.dependencies.insert(package.clone());
        }
        Ok(s::TypeId {
            package_id: package,
            module: self.name(module.name)?,
            name: self.name(reference.name)?,
        })
    }
    /// Kinds are preserved for generics; unknown constructs are never inferred as Star.
    fn kind(&mut self, kind: &w::Kind, depth: usize) -> Result<s::Kind, Error> {
        self.charge(depth)?;
        match kind.sum.as_ref().ok_or(Error::UnsupportedType)? {
            w::kind::Sum::Star(_) => Ok(s::Kind::Star),
            w::kind::Sum::Nat(_) => Ok(s::Kind::Nat),
            w::kind::Sum::Arrow(arrow) => {
                if arrow.parameters.is_empty() {
                    return Err(Error::Definition);
                }
                Ok(s::Kind::Arrow {
                    parameters: arrow
                        .parameters
                        .iter()
                        .map(|p| self.kind(p, depth + 1))
                        .collect::<Result<_, _>>()?,
                    result: Box::new(
                        self.kind(arrow.result.as_deref().ok_or(Error::Reference)?, depth + 1)?,
                    ),
                })
            }
        }
    }
    /// Parameter order is meaningful; duplicate names make later substitution ambiguous.
    fn parameters(
        &mut self,
        params: &[w::Parameter],
        depth: usize,
    ) -> Result<Vec<s::Parameter>, Error> {
        let mut names = BTreeSet::new();
        params
            .iter()
            .map(|p| {
                self.charge(depth)?;
                let name = self.string(p.name)?;
                if !names.insert(name.clone()) {
                    return Err(Error::Definition);
                }
                Ok(s::Parameter {
                    name,
                    kind: self.kind(p.kind.as_ref().ok_or(Error::Reference)?, depth + 1)?,
                })
            })
            .collect()
    }
    /// Check table links once without expanding every unused tree. A chain of N interned
    /// nodes costs O(N), rather than O(N²); signature extraction still charges each expansion.
    fn interned_links(&mut self, ty: &w::Type, bound: usize, depth: usize) -> Result<(), Error> {
        self.charge(depth)?;
        let children: &[w::Type] = match ty.sum.as_ref().ok_or(Error::UnsupportedType)? {
            w::type_node::Sum::Interned(index) => {
                if usize::try_from(*index).map_err(|_| Error::Reference)? >= bound {
                    return Err(Error::Reference);
                }
                return Ok(());
            }
            w::type_node::Sum::Variable(v) => &v.arguments,
            w::type_node::Sum::Reference(v) | w::type_node::Sum::Synonym(v) => &v.arguments,
            w::type_node::Sum::Builtin(v) => &v.arguments,
            w::type_node::Sum::Nat(_) => return Ok(()),
            w::type_node::Sum::Forall(v) => {
                return self.interned_links(
                    v.body.as_deref().ok_or(Error::Reference)?,
                    bound,
                    depth + 1,
                );
            }
            w::type_node::Sum::Struct(v) => {
                for field in &v.fields {
                    self.interned_links(
                        field.value_type.as_ref().ok_or(Error::Reference)?,
                        bound,
                        depth + 1,
                    )?;
                }
                return Ok(());
            }
        };
        for child in children {
            self.interned_links(child, bound, depth + 1)?;
        }
        Ok(())
    }
    /// Read interned types only backwards through the table, so cycles cannot recurse forever.
    fn ty(&mut self, ty: &w::Type, bound: usize, depth: usize) -> Result<s::Type, Error> {
        self.charge(depth)?;
        let args = |this: &mut Self, args: &[w::Type]| {
            args.iter()
                .map(|t| this.ty(t, bound, depth + 1))
                .collect::<Result<Vec<_>, _>>()
        };
        match ty.sum.as_ref().ok_or(Error::UnsupportedType)? {
            w::type_node::Sum::Interned(index) => {
                let index = usize::try_from(*index).map_err(|_| Error::Reference)?;
                if index >= bound {
                    return Err(Error::Reference);
                }
                self.ty(
                    self.p.types.get(index).ok_or(Error::Reference)?,
                    index,
                    depth + 1,
                )
            }
            w::type_node::Sum::Variable(var) => Ok(s::Type::Variable {
                name: self.string(var.name)?,
                arguments: args(self, &var.arguments)?,
            }),
            w::type_node::Sum::Reference(named) => Ok(s::Type::Reference {
                target: self.reference(named.target.as_ref().ok_or(Error::Reference)?)?,
                arguments: args(self, &named.arguments)?,
            }),
            w::type_node::Sum::Synonym(named) => Ok(s::Type::Synonym {
                target: self.reference(named.target.as_ref().ok_or(Error::Reference)?)?,
                arguments: args(self, &named.arguments)?,
            }),
            w::type_node::Sum::Builtin(builtin) => {
                let name = match builtin.name {
                    0 => "unit",
                    1 => "bool",
                    2 => "int64",
                    3 => "date",
                    4 => "timestamp",
                    5 => "numeric",
                    6 => "party",
                    7 => "text",
                    8 => "contract_id",
                    9 => "optional",
                    10 => "list",
                    11 => "gen_map",
                    13 => "any",
                    14 => "any_exception",
                    15 => "type_rep",
                    16 => "arrow",
                    17 => "update",
                    18 => "failure_category",
                    19 => "text_map",
                    _ => return Err(Error::UnsupportedType),
                };
                Ok(s::Type::Builtin {
                    name: name.into(),
                    arguments: args(self, &builtin.arguments)?,
                })
            }
            w::type_node::Sum::Nat(value) => {
                let value = u8::try_from(*value)
                    .ok()
                    .filter(|n| *n <= 37)
                    .ok_or(Error::Definition)?;
                Ok(s::Type::Nat { value })
            }
            w::type_node::Sum::Struct(fields) => Ok(s::Type::Struct {
                fields: self.fields(&fields.fields, bound, depth + 1)?,
            }),
            w::type_node::Sum::Forall(forall) => {
                if forall.parameters.is_empty() {
                    return Err(Error::Definition);
                }
                Ok(s::Type::Forall {
                    parameters: self.parameters(&forall.parameters, depth + 1)?,
                    body: Box::new(self.ty(
                        forall.body.as_deref().ok_or(Error::Reference)?,
                        bound,
                        depth + 1,
                    )?),
                })
            }
        }
    }
    /// Preserve field/constructor order while rejecting duplicates before producing a signature.
    fn fields(
        &mut self,
        fields: &[w::Field],
        bound: usize,
        depth: usize,
    ) -> Result<Vec<s::Field>, Error> {
        let mut names = BTreeSet::new();
        fields
            .iter()
            .map(|f| {
                self.charge(depth)?;
                let name = self.string(f.name)?;
                if !names.insert(name.clone()) {
                    return Err(Error::Definition);
                }
                Ok(s::Field {
                    name,
                    value_type: self.ty(
                        f.value_type.as_ref().ok_or(Error::Reference)?,
                        bound,
                        depth + 1,
                    )?,
                })
            })
            .collect()
    }
    /// Choices describe signature/consuming behavior, not executable controllers or business logic.
    fn choices(&mut self, choices: &[w::Choice]) -> Result<Vec<s::Choice>, Error> {
        let mut names = BTreeSet::new();
        let mut result = Vec::new();
        for c in choices {
            self.charge(0)?;
            let name = self.string(c.name)?;
            if !names.insert(name.clone()) {
                return Err(Error::Definition);
            }
            let argument = c
                .argument
                .as_ref()
                .and_then(|a| a.value_type.as_ref())
                .ok_or(Error::Reference)?;
            result.push(s::Choice {
                name,
                consuming: c.consuming,
                argument_type: self.ty(argument, self.p.types.len(), 0)?,
                return_type: self.ty(
                    c.result.as_ref().ok_or(Error::Reference)?,
                    self.p.types.len(),
                    0,
                )?,
            });
        }
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }
    /// Construct a declaration identity in the current module/package.
    fn local(&mut self, module: &str, name: i32) -> Result<s::TypeId, Error> {
        Ok(s::TypeId {
            package_id: self.id.into(),
            module: module.into(),
            name: self.name(name)?,
        })
    }
}

/// Verify package-id spellings without accepting path-based identities or malformed hashes.
pub(crate) fn package_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Decode the full signature surface; external type references are resolved across packages later.
pub(crate) fn describe(decoded: &Decoded, budget: &mut Budget) -> Result<s::Package, Error> {
    let p = &decoded.package;
    if p.strings.len() > 250_000 || p.types.len() > 50_000 || p.modules.len() > 4096 {
        return Err(Error::Limit);
    }
    let mut r = Resolver {
        id: &decoded.id,
        p,
        budget,
        dependencies: BTreeSet::new(),
    };
    let metadata = p.metadata.as_ref().ok_or(Error::Definition)?;
    let name = r.string(metadata.name)?;
    let version = r.string(metadata.version)?;
    let upgraded = metadata
        .upgraded
        .as_ref()
        .map(|u| r.string(u.package_id))
        .transpose()?;
    if upgraded.as_ref().is_some_and(|p| !package_id(p)) {
        return Err(Error::Reference);
    }
    let mut data_types = Vec::new();
    let mut synonyms = Vec::new();
    let mut templates = Vec::new();
    let mut interfaces = Vec::new();
    let mut modules = BTreeSet::new();
    // Check all table links, but only expand types reachable from the signature surface.
    for (index, ty) in p.types.iter().enumerate() {
        r.interned_links(ty, index, 0)?;
    }
    for m in &p.modules {
        r.charge(0)?;
        let module = r.name(m.name)?;
        if !modules.insert(module.clone()) {
            return Err(Error::Definition);
        }
        for d in &m.data_types {
            r.charge(0)?;
            let id = r.local(&module, d.name)?;
            let definition = match d.definition.as_ref().ok_or(Error::Definition)? {
                w::data_type::Definition::Record(f) => s::Definition::Record {
                    fields: r.fields(&f.fields, p.types.len(), 0)?,
                },
                w::data_type::Definition::Variant(f) => s::Definition::Variant {
                    constructors: r.fields(&f.fields, p.types.len(), 0)?,
                },
                w::data_type::Definition::Enum(e) => {
                    let constructors = e
                        .constructors
                        .iter()
                        .map(|i| r.string(*i))
                        .collect::<Result<Vec<_>, _>>()?;
                    if !d.parameters.is_empty()
                        || constructors.iter().collect::<BTreeSet<_>>().len() != constructors.len()
                    {
                        return Err(Error::Definition);
                    }
                    s::Definition::Enum { constructors }
                }
                w::data_type::Definition::Interface(_) => s::Definition::Interface,
            };
            data_types.push(s::DataType {
                id,
                serializable: d.serializable,
                parameters: r.parameters(&d.parameters, 0)?,
                definition,
            });
        }
        for alias in &m.synonyms {
            r.charge(0)?;
            synonyms.push(s::Synonym {
                id: r.local(&module, alias.name)?,
                parameters: r.parameters(&alias.parameters, 0)?,
                value_type: r.ty(
                    alias.value_type.as_ref().ok_or(Error::Reference)?,
                    p.types.len(),
                    0,
                )?,
            });
        }
        for t in &m.templates {
            r.charge(0)?;
            templates.push(s::Template {
                id: r.local(&module, t.name)?,
                key_type: t
                    .key
                    .as_ref()
                    .map(|k| {
                        r.ty(
                            k.value_type.as_ref().ok_or(Error::Reference)?,
                            p.types.len(),
                            0,
                        )
                    })
                    .transpose()?,
                choices: r.choices(&t.choices)?,
                implements: t
                    .implements
                    .iter()
                    .map(|i| r.reference(i.interface.as_ref().ok_or(Error::Reference)?))
                    .collect::<Result<_, _>>()?,
            });
        }
        for i in &m.interfaces {
            r.charge(0)?;
            let mut methods = Vec::new();
            let mut names = BTreeSet::new();
            for method in &i.methods {
                let name = r.string(method.name)?;
                if !names.insert(name.clone()) {
                    return Err(Error::Definition);
                }
                methods.push(s::Field {
                    name,
                    value_type: r.ty(
                        method.value_type.as_ref().ok_or(Error::Reference)?,
                        p.types.len(),
                        0,
                    )?,
                });
            }
            methods.sort_by(|a, b| a.name.cmp(&b.name));
            interfaces.push(s::Interface {
                id: r.local(&module, i.name)?,
                view_type: r.ty(i.view.as_ref().ok_or(Error::Reference)?, p.types.len(), 0)?,
                methods,
                choices: r.choices(&i.choices)?,
                requires: i
                    .requires
                    .iter()
                    .map(|t| r.reference(t))
                    .collect::<Result<_, _>>()?,
            });
        }
    }
    data_types.sort_by(|a, b| a.id.cmp(&b.id));
    synonyms.sort_by(|a, b| a.id.cmp(&b.id));
    templates.sort_by(|a, b| a.id.cmp(&b.id));
    interfaces.sort_by(|a, b| a.id.cmp(&b.id));
    if data_types.windows(2).any(|w| w[0].id == w[1].id)
        || synonyms.windows(2).any(|w| w[0].id == w[1].id)
        || templates.windows(2).any(|w| w[0].id == w[1].id)
        || interfaces.windows(2).any(|w| w[0].id == w[1].id)
    {
        return Err(Error::Definition);
    }
    Ok(s::Package {
        package_id: decoded.id.clone(),
        name,
        version,
        lf_version: "2.1".into(),
        upgraded_package_id: upgraded,
        type_dependencies: r.dependencies.into_iter().collect(),
        data_types,
        synonyms,
        templates,
        interfaces,
    })
}

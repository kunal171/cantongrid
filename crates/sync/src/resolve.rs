//! Resolve only explicitly selected object paths. Named recursion stays finite because every
//! record traversal consumes a path segment; aliases and substitution have independent budgets.
use crate::SyncError as Error;
use cantongrid_metadata::schema::{DataType, Definition, Metadata, Synonym, Type, TypeId};
use cantongrid_models::FieldType;
use std::collections::BTreeMap;

/// Signature indexes retain exact package identities; names alone never resolve a record reference.
pub(crate) struct Resolver<'a> {
    types: BTreeMap<&'a TypeId, &'a DataType>,
    aliases: BTreeMap<&'a TypeId, &'a Synonym>,
    remaining: usize,
}
/// The projected scalar and its LF semantics are distinct: enum/ContractId outputs may both be
/// text, but changing enum cases or a ContractId's target still requires explicit review.
pub(crate) struct Selected {
    pub scalar: FieldType,
    pub semantics: serde_json::Value,
}
impl<'a> Resolver<'a> {
    /// Each comparison gets one bounded expansion budget across all selected expressions.
    pub fn new(metadata: &'a Metadata) -> Self {
        Self {
            types: metadata
                .packages
                .iter()
                .flat_map(|p| p.data_types.iter())
                .map(|d| (&d.id, d))
                .collect(),
            aliases: metadata
                .packages
                .iter()
                .flat_map(|p| p.synonyms.iter())
                .map(|d| (&d.id, d))
                .collect(),
            remaining: 100_000,
        }
    }
    /// Charge before copying/substituting nodes, including repeatedly referenced generic arguments.
    fn charge(&mut self, depth: usize) -> Result<(), Error> {
        if depth > 64 {
            return Err(Error::Limit);
        }
        self.remaining = self.remaining.checked_sub(1).ok_or(Error::Limit)?;
        Ok(())
    }
    /// Expand synonyms and closed parameters while preserving nominal references. For example,
    /// Amount n { value : Numeric n } applied to Nat(10) resolves value to Numeric 10 exactly.
    fn materialize(
        &mut self,
        ty: &Type,
        env: &BTreeMap<String, Type>,
        depth: usize,
    ) -> Result<Type, Error> {
        self.charge(depth)?;
        let arguments = |this: &mut Self, args: &[Type]| {
            args.iter()
                .map(|a| this.materialize(a, env, depth + 1))
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(match ty {
            Type::Variable { name, arguments } => {
                if !arguments.is_empty() {
                    return Err(Error::UnsupportedType);
                }
                let value = env.get(name).ok_or(Error::UnsupportedType)?;
                self.materialize(value, &BTreeMap::new(), depth + 1)?
            }
            Type::Synonym {
                target,
                arguments: args,
            } => {
                let alias = *self.aliases.get(target).ok_or(Error::Identity)?;
                if alias.parameters.len() != args.len() {
                    return Err(Error::UnsupportedType);
                }
                let values = arguments(self, args)?;
                let substitutions = alias
                    .parameters
                    .iter()
                    .map(|p| p.name.clone())
                    .zip(values)
                    .collect();
                self.materialize(&alias.value_type, &substitutions, depth + 1)?
            }
            Type::Reference {
                target,
                arguments: args,
            } => Type::Reference {
                target: target.clone(),
                arguments: arguments(self, args)?,
            },
            Type::Builtin {
                name,
                arguments: args,
            } => Type::Builtin {
                name: name.clone(),
                arguments: arguments(self, args)?,
            },
            Type::Nat { value } => Type::Nat { value: *value },
            _ => return Err(Error::UnsupportedType),
        })
    }
    /// Resolve a finite path through serializable records. Optional intermediate records are
    /// deliberately rejected until their exact PQS JSON encoding has an independent control.
    pub fn field(&mut self, template: &TypeId, path: &[String]) -> Result<Selected, Error> {
        if path.is_empty() || path.len() > 16 {
            return Err(Error::Limit);
        }
        let mut current = Type::Reference {
            target: template.clone(),
            arguments: vec![],
        };
        for part in path {
            self.charge(0)?;
            let Type::Reference { target, arguments } = current else {
                return Err(Error::UnsupportedType);
            };
            let declaration = *self.types.get(&target).ok_or(Error::Identity)?;
            if !declaration.serializable || declaration.parameters.len() != arguments.len() {
                return Err(Error::UnsupportedType);
            }
            let Definition::Record { fields } = &declaration.definition else {
                return Err(Error::UnsupportedType);
            };
            let selected = fields
                .iter()
                .find(|f| &f.name == part)
                .ok_or(Error::MissingField)?;
            let env = declaration
                .parameters
                .iter()
                .map(|p| p.name.clone())
                .zip(arguments)
                .collect();
            current = self.materialize(&selected.value_type, &env, 0)?;
        }
        self.scalar(&current, false)
    }
    /// Only supported exact scalars cross into model declarations. Lists/maps/variants require
    /// a future explicit projection vocabulary; they cannot silently become strings or JSON blobs.
    fn scalar(&mut self, ty: &Type, optional: bool) -> Result<Selected, Error> {
        self.charge(0)?;
        let (scalar, semantics) = match ty {
            Type::Builtin { name, arguments } => match (name.as_str(), arguments.as_slice()) {
                ("text", []) => (FieldType::Text, serde_json::json!({"builtin":"text"})),
                ("party", []) => (FieldType::Party, serde_json::json!({"builtin":"party"})),
                ("bool", []) => (FieldType::Bool, serde_json::json!({"builtin":"bool"})),
                ("int64", []) => (FieldType::Int64, serde_json::json!({"builtin":"int64"})),
                ("numeric", [Type::Nat { value }]) => (
                    FieldType::Numeric { scale: *value },
                    serde_json::json!({"numeric_scale":value}),
                ),
                ("optional", [item]) if !optional => {
                    let inner = self.scalar(item, true)?;
                    return Ok(Selected {
                        scalar: FieldType::Optional {
                            item: Box::new(inner.scalar),
                        },
                        semantics: serde_json::json!({"optional":inner.semantics}),
                    });
                }
                ("contract_id", [Type::Reference { target, arguments }])
                    if arguments.is_empty() =>
                {
                    (FieldType::Text, serde_json::json!({"contract_id":target}))
                }
                _ => return Err(Error::UnsupportedType),
            },
            Type::Reference { target, arguments } if arguments.is_empty() => {
                let declaration = self.types.get(target).ok_or(Error::Identity)?;
                let Definition::Enum { constructors } = &declaration.definition else {
                    return Err(Error::UnsupportedType);
                };
                (
                    FieldType::Text,
                    serde_json::json!({"enum":target,"constructors":constructors}),
                )
            }
            _ => return Err(Error::UnsupportedType),
        };
        Ok(Selected { scalar, semantics })
    }
}

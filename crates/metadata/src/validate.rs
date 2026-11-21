//! Resolve the extracted signature graph without executing LF or expanding recursive definitions.
use crate::{error::MetadataError as Error, schema::*};
use std::collections::{BTreeMap, BTreeSet};

/// Verify the referenced signature exists and is applied to no more formal arguments than it declares.
fn nominal(
    target: &TypeId,
    count: usize,
    types: &BTreeMap<&TypeId, &DataType>,
    interfaces: &BTreeSet<&TypeId>,
) -> Result<(), Error> {
    if let Some(declaration) = types.get(target) {
        if count > declaration.parameters.len() {
            return Err(Error::Reference);
        }
    } else if !interfaces.contains(target) || count != 0 {
        return Err(Error::Reference);
    }
    Ok(())
}

/// Recursion is already depth-bounded during decoding; nominal cycles remain references, not recursion.
fn ty(
    value: &Type,
    variables: &BTreeSet<&str>,
    types: &BTreeMap<&TypeId, &DataType>,
    aliases: &BTreeMap<&TypeId, &Synonym>,
    interfaces: &BTreeSet<&TypeId>,
) -> Result<(), Error> {
    let arguments = match value {
        Type::Reference { target, arguments } => {
            nominal(target, arguments.len(), types, interfaces)?;
            arguments
        }
        Type::Synonym { target, arguments } => {
            if aliases
                .get(target)
                .is_none_or(|a| a.parameters.len() != arguments.len())
            {
                return Err(Error::Reference);
            }
            arguments
        }
        Type::Variable { name, arguments } => {
            if !variables.contains(name.as_str()) {
                return Err(Error::Reference);
            }
            arguments
        }
        Type::Builtin { arguments, .. } => arguments,
        Type::Nat { .. } => return Ok(()),
        Type::Struct { fields } => {
            for field in fields {
                ty(&field.value_type, variables, types, aliases, interfaces)?;
            }
            return Ok(());
        }
        Type::Forall { parameters, body } => {
            let mut nested = variables.clone();
            for parameter in parameters {
                nested.insert(&parameter.name);
            }
            return ty(body, &nested, types, aliases, interfaces);
        }
    };
    for argument in arguments {
        ty(argument, variables, types, aliases, interfaces)?;
    }
    Ok(())
}

/// Reject dangling cross-package references and ambiguous declaration/link identities.
/// This validates metadata links, not the semantics or authorization of executable LF expressions.
pub(crate) fn validate(packages: &[Package]) -> Result<(), Error> {
    let ids: BTreeSet<_> = packages.iter().map(|p| p.package_id.as_str()).collect();
    if ids.len() != packages.len() {
        return Err(Error::Definition);
    }
    let types: BTreeMap<_, _> = packages
        .iter()
        .flat_map(|p| p.data_types.iter())
        .map(|d| (&d.id, d))
        .collect();
    let aliases: BTreeMap<_, _> = packages
        .iter()
        .flat_map(|p| p.synonyms.iter())
        .map(|d| (&d.id, d))
        .collect();
    let interfaces: BTreeSet<_> = packages
        .iter()
        .flat_map(|p| p.interfaces.iter())
        .map(|i| &i.id)
        .collect();
    let closed = BTreeSet::new();
    for package in packages {
        if package
            .type_dependencies
            .iter()
            .any(|id| !ids.contains(id.as_str()))
        {
            return Err(Error::Reference);
        }
        for declaration in &package.data_types {
            let variables = declaration
                .parameters
                .iter()
                .map(|p| p.name.as_str())
                .collect();
            let fields = match &declaration.definition {
                Definition::Record { fields } => fields,
                Definition::Variant { constructors } => constructors,
                _ => continue,
            };
            for field in fields {
                ty(&field.value_type, &variables, &types, &aliases, &interfaces)?;
            }
        }
        for alias in &package.synonyms {
            let variables = alias.parameters.iter().map(|p| p.name.as_str()).collect();
            ty(&alias.value_type, &variables, &types, &aliases, &interfaces)?;
        }
        for template in &package.templates {
            let payload = types.get(&template.id).ok_or(Error::Reference)?;
            if !matches!(payload.definition, Definition::Record { .. })
                || !payload.serializable
                || !payload.parameters.is_empty()
            {
                return Err(Error::Definition);
            }
            if let Some(key) = &template.key_type {
                ty(key, &closed, &types, &aliases, &interfaces)?;
            }
            for choice in &template.choices {
                ty(
                    &choice.argument_type,
                    &closed,
                    &types,
                    &aliases,
                    &interfaces,
                )?;
                ty(&choice.return_type, &closed, &types, &aliases, &interfaces)?;
            }
            if template
                .implements
                .iter()
                .any(|id| !interfaces.contains(id))
            {
                return Err(Error::Reference);
            }
            if template.implements.iter().collect::<BTreeSet<_>>().len()
                != template.implements.len()
            {
                return Err(Error::Definition);
            }
        }
        for interface in &package.interfaces {
            ty(&interface.view_type, &closed, &types, &aliases, &interfaces)?;
            for method in &interface.methods {
                ty(&method.value_type, &closed, &types, &aliases, &interfaces)?;
            }
            for choice in &interface.choices {
                ty(
                    &choice.argument_type,
                    &closed,
                    &types,
                    &aliases,
                    &interfaces,
                )?;
                ty(&choice.return_type, &closed, &types, &aliases, &interfaces)?;
            }
            if interface.requires.iter().any(|id| !interfaces.contains(id)) {
                return Err(Error::Reference);
            }
            if interface.requires.iter().collect::<BTreeSet<_>>().len() != interface.requires.len()
            {
                return Err(Error::Definition);
            }
        }
    }
    Ok(())
}

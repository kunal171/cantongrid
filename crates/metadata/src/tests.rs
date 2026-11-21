//! Synthetic signature packages exercise wire/ZIP rejection controls without a Ledger or SDK runtime.
use super::*;
use crate::{schema as s, wire as w};
use prost::Message;
use std::io::{Cursor, Write};
use zip::{ZipWriter, write::SimpleFileOptions};

/// Build a tiny package with a stable string/name table and explicit package metadata.
fn package(name: &str) -> w::Package {
    let mut p = w::Package::default();
    let name = string(&mut p, name);
    let version = string(&mut p, "1.0.0");
    p.metadata = Some(w::PackageMetadata {
        name,
        version,
        upgraded: None,
    });
    p
}
/// Intern one test declaration string, preserving source order.
fn string(p: &mut w::Package, value: &str) -> i32 {
    p.strings.push(value.into());
    (p.strings.len() - 1) as i32
}
/// Build the same dotted-name encoding used by actual LF packages.
fn name(p: &mut w::Package, value: &str) -> i32 {
    let segments = value.split('.').map(|s| string(p, s)).collect();
    p.dotted_names.push(w::DottedName { segments });
    (p.dotted_names.len() - 1) as i32
}
/// Scalar/constructor applications retain ordered type arguments.
fn builtin(name: i32, arguments: Vec<w::Type>) -> w::Type {
    w::Type {
        sum: Some(w::type_node::Sum::Builtin(w::Builtin { name, arguments })),
    }
}
/// Create a correctly qualified reference to a self or imported type.
fn reference(p: &mut w::Package, package: Option<&str>, module: &str, ty: &str) -> w::TypeRef {
    let sum = match package {
        None => w::package_ref::Sum::SelfPackage(w::Unit {}),
        Some(id) => w::package_ref::Sum::Imported(string(p, id)),
    };
    w::TypeRef {
        module: Some(w::ModuleRef {
            package: Some(w::PackageRef { sum: Some(sum) }),
            name: name(p, module),
        }),
        name: name(p, ty),
    }
}
/// Named references are not expanded, allowing legitimate recursive records.
fn nominal(target: w::TypeRef) -> w::Type {
    w::Type {
        sum: Some(w::type_node::Sum::Reference(w::Named {
            target: Some(target),
            arguments: vec![],
        })),
    }
}
/// Field helpers preserve the declaration-order metadata contract.
fn field(p: &mut w::Package, label: &str, value_type: w::Type) -> w::Field {
    w::Field {
        name: string(p, label),
        value_type: Some(value_type),
    }
}
/// DALF identity hashes original payload bytes, not decoded/re-encoded declarations.
fn dalf(p: &w::Package, minor: &str) -> (Vec<u8>, String) {
    let payload = w::Payload {
        minor: minor.into(),
        patch: 0,
        sum: Some(w::payload::Sum::Lf2(p.encode_to_vec())),
    }
    .encode_to_vec();
    let hash = decode::hex(&Sha256::digest(&payload));
    (
        w::Archive {
            hash_function: 0,
            payload,
            hash: hash.clone(),
        }
        .encode_to_vec(),
        hash,
    )
}
/// Produce a bounded in-memory ZIP fixture; no fixture extraction writes to disk.
fn zip(entries: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, body) in entries {
        writer
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(&body).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
/// Generate the explicit package-list manifest used by the strict reader.
fn manifest(main: &str, packages: &[&str]) -> Vec<u8> {
    format!("Manifest-Version: 1.0\nFormat: daml-lf\nEncryption: non-encrypted\nMain-Dalf: {main}\nDalfs: {}\n",packages.join(", ")).into_bytes()
}
/// A real dependency-defined record plus a template containing generic/recursive type structure.
fn fixture() -> (Vec<u8>, String) {
    let mut dependency = package("shared");
    let m = name(&mut dependency, "Shared");
    let record = name(&mut dependency, "Amount");
    let scale = w::Type {
        sum: Some(w::type_node::Sum::Nat(10)),
    };
    let fields = vec![field(&mut dependency, "value", builtin(5, vec![scale]))];
    dependency.modules.push(w::Module {
        name: m,
        data_types: vec![w::DataType {
            name: record,
            serializable: true,
            parameters: vec![],
            definition: Some(w::data_type::Definition::Record(w::Fields { fields })),
        }],
        ..Default::default()
    });
    let (dep, id) = dalf(&dependency, "1");
    let mut p = package("application");
    let module = name(&mut p, "Example");
    let record = name(&mut p, "Record");
    let shared = nominal(reference(&mut p, Some(&id), "Shared", "Amount"));
    let recursive = nominal(reference(&mut p, None, "Example", "Record"));
    let fields = vec![
        field(&mut p, "amount", shared),
        field(&mut p, "next", builtin(9, vec![recursive])),
        field(&mut p, "labels", builtin(10, vec![builtin(7, vec![])])),
    ];
    let choice = w::Choice {
        name: string(&mut p, "Archive"),
        consuming: true,
        argument: Some(w::Argument {
            value_type: Some(builtin(0, vec![])),
        }),
        result: Some(builtin(0, vec![])),
    };
    let enum_name = name(&mut p, "Outcome");
    let constructors = vec![string(&mut p, "Yes"), string(&mut p, "No")];
    let variant_name = name(&mut p, "Decision");
    let variants = vec![
        field(&mut p, "Keep", builtin(0, vec![])),
        field(&mut p, "Replace", builtin(7, vec![])),
    ];
    p.modules.push(w::Module {
        name: module,
        data_types: vec![
            w::DataType {
                name: record,
                serializable: true,
                parameters: vec![],
                definition: Some(w::data_type::Definition::Record(w::Fields { fields })),
            },
            w::DataType {
                name: enum_name,
                serializable: true,
                parameters: vec![],
                definition: Some(w::data_type::Definition::Enum(w::Enum { constructors })),
            },
            w::DataType {
                name: variant_name,
                serializable: true,
                parameters: vec![],
                definition: Some(w::data_type::Definition::Variant(w::Fields {
                    fields: variants,
                })),
            },
        ],
        templates: vec![w::Template {
            name: record,
            choices: vec![choice],
            ..Default::default()
        }],
        ..Default::default()
    });
    let (main, main_id) = dalf(&p, "1");
    (
        zip(vec![
            (
                "META-INF/MANIFEST.MF",
                manifest("main.dalf", &["main.dalf", "dep.dalf"]),
            ),
            ("main.dalf", main),
            ("dep.dalf", dep),
        ]),
        main_id,
    )
}

/// Named dependency types, recursive records, variants/enums and exact Numeric scale survive extraction.
#[test]
fn signatures_preserve_dependencies_and_are_deterministic() {
    let (bytes, id) = fixture();
    let metadata = inspect(&bytes).unwrap();
    assert_eq!(metadata.main_package_id, id);
    assert_eq!(metadata.packages.len(), 2);
    assert_eq!(
        metadata.to_json().unwrap(),
        inspect(&bytes).unwrap().to_json().unwrap()
    );
    let main = metadata
        .packages
        .iter()
        .find(|p| p.package_id == id)
        .unwrap();
    assert_eq!(main.type_dependencies.len(), 1);
    assert_eq!(main.templates[0].choices[0].name, "Archive");
    assert!(main.templates[0].choices[0].consuming);
    assert!(
        main.data_types
            .iter()
            .any(|d| matches!(d.definition, s::Definition::Variant { .. }))
    );
    assert!(
        main.data_types
            .iter()
            .any(|d| matches!(d.definition, s::Definition::Enum { .. }))
    );
    let dep = metadata
        .packages
        .iter()
        .find(|p| p.name == "shared")
        .unwrap();
    let s::Definition::Record { fields } = &dep.data_types[0].definition else {
        panic!("record");
    };
    let s::Type::Builtin { name, arguments } = &fields[0].value_type else {
        panic!("numeric");
    };
    assert_eq!(name, "numeric");
    assert_eq!(arguments, &vec![s::Type::Nat { value: 10 }]);
}

/// Hash/version failures are rejected before metadata or deployment identities can be trusted.
#[test]
fn corrupted_packages_and_unknown_versions_fail_closed() {
    let p = package("empty");
    for minor in ["2", "999", "dev"] {
        let (bytes, _) = dalf(&p, minor);
        assert!(matches!(
            decode::decode(&bytes),
            Err(MetadataError::UnsupportedVersion)
        ));
    }
    let (bytes, _) = dalf(&p, "1");
    let mut archive = w::Archive::decode(bytes.as_slice()).unwrap();
    archive.hash = "a".repeat(64);
    assert!(matches!(
        decode::decode(&archive.encode_to_vec()),
        Err(MetadataError::Hash)
    ));
    archive.hash_function = 1;
    assert!(matches!(
        decode::decode(&archive.encode_to_vec()),
        Err(MetadataError::Hash)
    ));
    assert!(inspect(b"not a zip archive").is_err());
}

/// Imported type links cannot silently disappear or refer to missing packages/declarations.
#[test]
fn missing_or_unresolved_type_dependencies_are_rejected() {
    let (bytes, _) = fixture();
    let mut c = container::read(&bytes).unwrap();
    c.dalfs.remove("dep.dalf");
    let main = c.dalfs.remove("main.dalf").unwrap();
    let broken = zip(vec![
        (
            "META-INF/MANIFEST.MF",
            manifest("main.dalf", &["main.dalf"]),
        ),
        ("main.dalf", main),
    ]);
    assert_eq!(inspect(&broken).unwrap_err(), MetadataError::Reference);
    let mut metadata = inspect(&bytes).unwrap();
    let dep = metadata
        .packages
        .iter_mut()
        .find(|p| p.name == "shared")
        .unwrap();
    dep.data_types.clear();
    assert_eq!(
        validate::validate(&metadata.packages),
        Err(MetadataError::Reference)
    );
}

/// Interned types must reference earlier entries and stay inside depth/expansion budgets.
#[test]
fn invalid_interning_and_deep_expansion_are_rejected() {
    let mut p = package("invalid");
    p.types.push(w::Type {
        sum: Some(w::type_node::Sum::Interned(0)),
    });
    let (bytes, _) = dalf(&p, "1");
    let decoded = decode::decode(&bytes).unwrap();
    assert_eq!(
        decode::describe(&decoded, &mut decode::Budget::new()).unwrap_err(),
        MetadataError::Reference
    );
    p.types = vec![builtin(7, vec![])];
    for i in 0..70 {
        p.types.push(w::Type {
            sum: Some(w::type_node::Sum::Interned(i)),
        });
    }
    attach_type(
        &mut p,
        w::Type {
            sum: Some(w::type_node::Sum::Interned(70)),
        },
    );
    let (bytes, _) = dalf(&p, "1");
    let decoded = decode::decode(&bytes).unwrap();
    assert_eq!(
        decode::describe(&decoded, &mut decode::Budget::new()).unwrap_err(),
        MetadataError::Limit
    );
    p.types = vec![builtin(98765, vec![])];
    p.modules.clear();
    attach_type(
        &mut p,
        w::Type {
            sum: Some(w::type_node::Sum::Interned(0)),
        },
    );
    let (bytes, _) = dalf(&p, "1");
    let decoded = decode::decode(&bytes).unwrap();
    assert_eq!(
        decode::describe(&decoded, &mut decode::Budget::new()).unwrap_err(),
        MetadataError::UnsupportedType
    );
}

/// Resource checks reject oversized input/count before collecting archive entries or parsing LF.
#[test]
fn archive_resource_limits_are_bounded() {
    assert_eq!(
        inspect(&vec![0; container::MAX_INPUT + 1]).unwrap_err(),
        MetadataError::Limit
    );
    let names: Vec<_> = (0..1025).map(|i| format!("file-{i}")).collect();
    let bytes = zip(names.iter().map(|n| (n.as_str(), vec![])).collect());
    assert_eq!(inspect(&bytes).unwrap_err(), MetadataError::Limit);
    let bytes = zip(vec![("../main.dalf", vec![])]);
    assert_eq!(inspect(&bytes).unwrap_err(), MetadataError::Archive);
    let bytes = zip(vec![(
        "META-INF/MANIFEST.MF",
        vec![b'x'; manifest::MAX_MANIFEST_BYTES + 1],
    )]);
    assert_eq!(inspect(&bytes).unwrap_err(), MetadataError::Limit);
}

/// Duplicate archive labels cannot pick one of two contradictory package contents.
#[test]
fn duplicate_paths_and_manifest_package_mismatches_are_rejected() {
    let bytes = zip(vec![("main.dalf", vec![1]), ("copy.dalf", vec![2])]);
    // Rewrite equal-length local/central labels without altering either entry's data/CRC.
    let mut duplicate = bytes;
    for i in 0..duplicate.len() - 8 {
        if &duplicate[i..i + 9] == b"copy.dalf" {
            duplicate[i..i + 9].copy_from_slice(b"main.dalf");
        }
    }
    assert_eq!(inspect(&duplicate).unwrap_err(), MetadataError::Archive);
    let bytes = zip(vec![
        (
            "META-INF/MANIFEST.MF",
            manifest("main.dalf", &["main.dalf"]),
        ),
        ("main.dalf", vec![]),
        ("unlisted.dalf", vec![]),
    ]);
    assert_eq!(inspect(&bytes).unwrap_err(), MetadataError::Manifest);
}

/// Put a type on the extracted surface, so tests exercise reachable expansion rather than execution-only tables.
fn attach_type(p: &mut w::Package, value_type: w::Type) {
    let module = name(p, "Surface");
    let record = name(p, "Value");
    let fields = vec![field(p, "value", value_type)];
    p.modules.push(w::Module {
        name: module,
        data_types: vec![w::DataType {
            name: record,
            serializable: true,
            parameters: vec![],
            definition: Some(w::data_type::Definition::Record(w::Fields { fields })),
        }],
        ..Default::default()
    });
}

/// The upstream InterfaceMethod has Location at tag 1 and its identifier at tag 2.
/// Fixed bytes, independent of our encoder, guard against accidentally swapping these tags.
#[test]
fn interface_method_tags_follow_upstream_schema() {
    let method = w::Method::decode(&b"\x0a\x00\x10\x07\x1a\x04\x1a\x02\x08\x07"[..]).unwrap();
    assert_eq!(method.name, 7);
    assert_eq!(method.value_type, Some(builtin(7, vec![])));
}

/// Generic aliases and interface methods retain variables, links and opaque ContractId targets.
#[test]
fn generics_aliases_and_interface_signatures_remain_qualified() {
    let mut p = package("interfaces");
    let variable = string(&mut p, "a");
    let parameter = w::Parameter {
        name: variable,
        kind: Some(w::Kind {
            sum: Some(w::kind::Sum::Star(w::Unit {})),
        }),
    };
    let variable_type = w::Type {
        sum: Some(w::type_node::Sum::Variable(w::Variable {
            name: variable,
            arguments: vec![],
        })),
    };
    let alias_name = name(&mut p, "Identity");
    let interface_name = name(&mut p, "Readable");
    let module_name = name(&mut p, "Example");
    let target = reference(&mut p, None, "Example", "Readable");
    let method_name = string(&mut p, "label");
    p.modules.push(w::Module {
        name: module_name,
        synonyms: vec![w::Synonym {
            name: alias_name,
            parameters: vec![parameter],
            value_type: Some(variable_type),
        }],
        interfaces: vec![w::Interface {
            name: interface_name,
            methods: vec![w::Method {
                name: method_name,
                value_type: Some(builtin(7, vec![])),
            }],
            view: Some(builtin(0, vec![])),
            ..Default::default()
        }],
        ..Default::default()
    });
    let alias_target = reference(&mut p, None, "Example", "Identity");
    let alias = w::Type {
        sum: Some(w::type_node::Sum::Synonym(w::Named {
            target: Some(alias_target),
            arguments: vec![builtin(7, vec![])],
        })),
    };
    attach_type(
        &mut p,
        builtin(11, vec![alias, builtin(8, vec![nominal(target)])]),
    );
    let (dalf, _) = dalf(&p, "1");
    let bytes = zip(vec![
        (
            "META-INF/MANIFEST.MF",
            manifest("main.dalf", &["main.dalf"]),
        ),
        ("main.dalf", dalf),
    ]);
    let metadata = inspect(&bytes).unwrap();
    let main = &metadata.packages[0];
    assert_eq!(main.interfaces[0].methods[0].name, "label");
    assert_eq!(main.synonyms[0].parameters[0].name, "a");
    assert!(matches!(
        main.synonyms[0].value_type,
        s::Type::Variable { .. }
    ));
    let mut broken = metadata.clone();
    broken.packages[0].interfaces.clear();
    assert_eq!(
        validate::validate(&broken.packages),
        Err(MetadataError::Reference)
    );
    broken = metadata;
    broken.packages[0].synonyms[0].parameters.clear();
    assert_eq!(
        validate::validate(&broken.packages),
        Err(MetadataError::Reference)
    );
}

/// Header disagreement and CRC errors cannot produce trusted metadata from alternate ZIP views.
#[test]
fn archive_header_disagreement_and_crc_corruption_are_rejected() {
    let (bytes, _) = fixture();
    let mut disagreement = bytes.clone();
    let offset = disagreement
        .windows(9)
        .position(|w| w == b"main.dalf")
        .unwrap();
    disagreement[offset] = b'x'; // Change only the first (local) name, retaining the central directory.
    assert_eq!(inspect(&disagreement).unwrap_err(), MetadataError::Archive);
    let mut crc = bytes;
    let offset = crc
        .windows(21)
        .position(|w| w == b"Manifest-Version: 1.0")
        .unwrap();
    crc[offset] ^= 1; // Stored entry content changes without updating its CRC.
    assert_eq!(inspect(&crc).unwrap_err(), MetadataError::Archive);
}

/// Reordering archive entries changes artifact identity, never the logical package inventory.
#[test]
fn archive_order_does_not_change_signature_order() {
    let (bytes, _) = fixture();
    let metadata = inspect(&bytes).unwrap();
    let container = container::read(&bytes).unwrap();
    let mut entries = vec![(
        "META-INF/MANIFEST.MF",
        manifest("main.dalf", &["main.dalf", "dep.dalf"]),
    )];
    entries.extend(
        container
            .dalfs
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.clone())),
    );
    let reordered = inspect(&zip(entries)).unwrap();
    assert_eq!(metadata.main_package_id, reordered.main_package_id);
    assert_eq!(metadata.packages, reordered.packages);
    assert_ne!(metadata.artifact_sha256, reordered.artifact_sha256);
}

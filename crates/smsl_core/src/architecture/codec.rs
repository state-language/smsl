use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;

use super::{SystemDefinition, catalog, error, validate};
use crate::{
    Diagnostic, encoding,
    source::{Resolver, Source, SourceFormat, SourceText},
};

pub fn decode(source: &str, format: SourceFormat) -> Result<SystemDefinition, Diagnostic> {
    let value = match format.encoding() {
        Some(format) => encoding::decode(source, format)?,
        None => {
            let parsed = crate::source::native::parse(source)?;
            if parsed.kind != "system" {
                return Err(error("system_syntax", "Expected a system declaration"));
            }
            parsed.value
        }
    };
    serde_json::from_value(value).map_err(|cause| error("system_syntax", cause.to_string()))
}

pub fn encode(definition: &SystemDefinition, format: SourceFormat) -> Result<String, Diagnostic> {
    let value = serde_json::to_value(definition).expect("system serialization");
    match format.encoding() {
        Some(format) => encoding::encode(&value, format),
        None => crate::source::native::encode_system(definition),
    }
}

pub fn link(source: Source, resolver: &mut impl Resolver) -> Result<SystemDefinition, Diagnostic> {
    let mut active = BTreeSet::new();
    let mut loaded = BTreeSet::new();
    let mut result = decode(&source.text, source.format)?;
    result.components.clear();
    result.contracts.clear();
    result.connections.clear();
    result.aliases.clear();
    result.imports.clear();
    merge(source, resolver, &mut active, &mut loaded, &mut result)?;
    validate(&result)?;
    Ok(result)
}

fn merge(
    source: Source,
    resolver: &mut impl Resolver,
    active: &mut BTreeSet<String>,
    loaded: &mut BTreeSet<String>,
    result: &mut SystemDefinition,
) -> Result<(), Diagnostic> {
    if active.contains(&source.id) {
        return Err(error("import_cycle", &source.id));
    }
    if loaded.contains(&source.id) {
        return Ok(());
    }
    if active.len() >= 96 || loaded.len() >= 4096 {
        return Err(error(
            "resource_limit",
            "System imports exceed depth or module limit",
        ));
    }
    active.insert(source.id.clone());
    let definition = decode(&source.text, source.format)?;
    if definition.system_version != result.system_version {
        return Err(error(
            "system_version",
            "Imported system schema version differs",
        ));
    }
    for import in &definition.imports {
        merge(
            resolver.resolve(&source.id, import)?,
            resolver,
            active,
            loaded,
            result,
        )?;
    }
    for (key, value) in definition.aliases {
        if let Some(previous) = result.aliases.insert(key.clone(), value.clone()) {
            if previous != value {
                return Err(error("conflicting_alias", key));
            }
        }
    }
    result.components.extend(definition.components);
    result.contracts.extend(definition.contracts);
    result.connections.extend(definition.connections);
    active.remove(&source.id);
    loaded.insert(source.id);
    Ok(())
}

pub fn link_sources(
    entry: &str,
    sources: &BTreeMap<String, SourceText>,
) -> Result<SystemDefinition, Diagnostic> {
    let read = |id: String| {
        let source = sources
            .get(&id)
            .ok_or_else(|| error("invalid_source", format!("Missing source {id:?}")))?;
        Ok(Source {
            id,
            text: source.text.clone(),
            format: source.format,
        })
    };
    link(
        read(normalize(entry)?)?,
        &mut |importer: &str, source: &str| {
            let path = if source.starts_with('/') {
                source.to_owned()
            } else if let Some((parent, _)) = importer.rsplit_once('/') {
                format!("{parent}/{source}")
            } else {
                source.to_owned()
            };
            read(normalize(&path)?)
        },
    )
}

fn normalize(path: &str) -> Result<String, Diagnostic> {
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts
                    .pop()
                    .ok_or_else(|| error("invalid_source", "Import escapes source root"))?;
            }
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        return Err(error("invalid_source", "Source identity must name a file"));
    }
    Ok(format!(
        "{}{}",
        if path.starts_with('/') { "/" } else { "" },
        parts.join("/")
    ))
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    RegisterProvider {
        providers: Vec<crate::system::ProviderDescription>,
        provider: crate::system::ProviderDescription,
    },
    GetProvider {
        providers: Vec<crate::system::ProviderDescription>,
        id: String,
    },
    DescribeProvider {
        id: String,
        contracts: Vec<String>,
        #[serde(default = "empty_object")]
        metadata: Value,
    },
    ValidateProviders {
        definition: SystemDefinition,
        providers: Vec<crate::system::ProviderDescription>,
    },
    InspectProviders {
        providers: Vec<crate::system::ProviderDescription>,
        contract: Option<String>,
    },
    CheckProvider {
        providers: Vec<crate::system::ProviderDescription>,
        id: String,
        contract: String,
    },
    Catalog {},
    Decode {
        source: String,
        format: SourceFormat,
    },
    Encode {
        definition: SystemDefinition,
        format: SourceFormat,
    },
    Validate {
        definition: SystemDefinition,
    },
    Link {
        entry: String,
        sources: BTreeMap<String, SourceText>,
    },
}

pub fn request(source: &str) -> Result<String, Diagnostic> {
    let value = encoding::decode(source, encoding::Encoding::Json)?;
    let request: Request = serde_json::from_value(value)
        .map_err(|cause| error("system_request", cause.to_string()))?;
    let result: Value = match request {
        Request::RegisterProvider {
            providers,
            provider,
        } => {
            let mut catalog = crate::system::ProviderCatalog::new(providers)?;
            catalog.register(provider)?;
            serde_json::to_value(catalog.inspect(None))
        }
        Request::GetProvider { providers, id } => {
            serde_json::to_value(crate::system::ProviderCatalog::new(providers)?.describe(&id)?)
        }
        Request::DescribeProvider {
            id,
            contracts,
            metadata,
        } => serde_json::to_value(crate::system::describe_provider(id, contracts, metadata)?),
        Request::ValidateProviders {
            definition,
            providers,
        } => serde_json::to_value(
            crate::system::ProviderCatalog::new(providers)?.validate_bindings(&definition)?,
        ),
        Request::InspectProviders {
            providers,
            contract,
        } => serde_json::to_value(
            crate::system::ProviderCatalog::new(providers)?.inspect(contract.as_deref()),
        ),
        Request::CheckProvider {
            providers,
            id,
            contract,
        } => serde_json::to_value(
            crate::system::ProviderCatalog::new(providers)?.check(&id, &contract)?,
        ),
        Request::Catalog {} => serde_json::to_value(catalog()),
        Request::Decode { source, format } => serde_json::to_value(decode(&source, format)?),
        Request::Encode { definition, format } => {
            serde_json::to_value(encode(&definition, format)?)
        }
        Request::Validate { definition } => serde_json::to_value(validate(&definition)?),
        Request::Link { entry, sources } => serde_json::to_value(link_sources(&entry, &sources)?),
    }
    .expect("system response serialization");
    Ok(serde_json::to_string(&result).expect("system response JSON"))
}

fn empty_object() -> Value {
    serde_json::json!({})
}

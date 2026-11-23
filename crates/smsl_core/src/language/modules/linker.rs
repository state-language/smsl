use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Diagnostic, Machine, compile, definition,
    model::{Branch, Document, EventDeclaration},
};

use super::{Domain, Instance, Template, decode};
use crate::source::{Resolver, Source};

pub fn link(root: Source, resolver: &mut impl Resolver) -> Result<Machine, Diagnostic> {
    compile(link_document(root, resolver)?)
}

pub fn link_document(root: Source, resolver: &mut impl Resolver) -> Result<Document, Diagnostic> {
    let mut linker = Linker {
        resolver,
        sources: BTreeMap::new(),
        requests: BTreeMap::new(),
        exports: BTreeMap::new(),
        branches: BTreeMap::new(),
        identities: BTreeSet::new(),
        templates: BTreeMap::new(),
        domains: BTreeMap::new(),
        domain_bytes: 0,
        instances: Vec::new(),
        events: Vec::new(),
        roots: BTreeSet::new(),
        version: None,
    };
    linker.visit(root, 0)?;
    let mut expanded_bytes = 0usize;
    for instance in &linker.instances {
        let (template, dependencies) = linker
            .templates
            .get(&instance.template)
            .ok_or_else(|| error(format!("Unknown branch template {:?}", instance.template)))?;
        let branch = super::templates::instantiate(template, &instance.name, &instance.arguments)?;
        let bytes = serde_json::to_vec(&branch)
            .expect("branch definition")
            .len();
        expanded_bytes = expanded_bytes
            .checked_add(bytes)
            .ok_or_else(|| error("Expanded instances exceed resource budget"))?;
        if expanded_bytes > crate::limits::MAX_INPUT_BYTES {
            return Err(Diagnostic::new(
                "resource_limit",
                "Expanded instances exceed 8 MiB",
                None,
            ));
        }
        let dependencies = dependencies
            .iter()
            .filter(|name| *name != &instance.template)
            .cloned()
            .collect();
        linker
            .branches
            .insert(instance.name.clone(), (branch, dependencies));
    }
    let mut pending: Vec<_> = linker.roots.into_iter().collect();
    let mut included = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if included.insert(name.clone()) {
            if linker.templates.contains_key(&name) || linker.domains.contains_key(&name) {
                continue;
            }
            let branch = linker
                .branches
                .get(&name)
                .ok_or_else(|| error(format!("Unknown instantiated branch {name:?}")))?;
            pending.extend(branch.1.iter().cloned());
        }
    }
    let document = Document {
        version: linker.version.expect("root module loaded"),
        events: linker.events,
        branches: linker
            .branches
            .into_iter()
            .filter_map(|(name, (branch, _))| included.contains(&name).then_some(branch))
            .collect(),
    };
    let document = definition::compose(&[document])?;
    let prototypes = linker
        .instances
        .iter()
        .filter(|instance| included.contains(&instance.name))
        .map(|instance| {
            let (template, _) = &linker.templates[&instance.template];
            Ok((
                super::templates::prototype(template, &instance.name)?,
                template.parameters.as_slice(),
            ))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    crate::language::validation::memory::validate_templates(&document, &prototypes)?;
    Ok(document)
}

type Symbols = BTreeMap<String, String>;

struct Linker<'a, R> {
    resolver: &'a mut R,
    sources: BTreeMap<String, (usize, Source)>,
    requests: BTreeMap<(String, String), Source>,
    exports: BTreeMap<String, Symbols>,
    branches: BTreeMap<String, (Branch, BTreeSet<String>)>,
    identities: BTreeSet<String>,
    templates: BTreeMap<String, (Template, BTreeSet<String>)>,
    domains: BTreeMap<String, Domain>,
    domain_bytes: usize,
    instances: Vec<Instance>,
    events: Vec<EventDeclaration>,
    roots: BTreeSet<String>,
    version: Option<String>,
}

impl<R: Resolver> Linker<'_, R> {
    fn visit(&mut self, source: Source, depth: usize) -> Result<Symbols, Diagnostic> {
        if depth > crate::limits::MAX_NESTING {
            return Err(Diagnostic::new(
                "resource_limit",
                "Module import depth exceeds 96 levels",
                None,
            ));
        }
        if source.id.is_empty() {
            return Err(error("Resolved module identity must be nonempty"));
        }
        if let Some((_, previous)) = self.sources.get(&source.id) {
            if previous != &source {
                return Err(error(format!(
                    "Module identity {:?} resolved to conflicting contents",
                    source.id
                )));
            }
            return self
                .exports
                .get(&source.id)
                .cloned()
                .ok_or_else(|| error(format!("Import cycle reaches {:?}", source.id)));
        }
        let index = self.sources.len();
        self.sources
            .insert(source.id.clone(), (index, source.clone()));
        let module =
            decode(&source.text, source.format).map_err(|error| in_source(error, &source.id))?;
        match &self.version {
            Some(version) if version != &module.version => {
                return Err(error(format!(
                    "Module {:?} uses a different language version",
                    source.id
                )));
            }
            None => self.version = Some(module.version.clone()),
            _ => {}
        }
        let mut symbols = Symbols::new();
        for local in module
            .branches
            .iter()
            .map(|branch| &branch.name)
            .chain(
                module
                    .templates
                    .iter()
                    .map(|template| &template.branch.name),
            )
            .chain(module.instances.iter().map(|instance| &instance.name))
            .chain(module.domains.iter().map(|domain| &domain.name))
        {
            let name = if index == 0 {
                local.clone()
            } else {
                format!("module{index}.{local}")
            };
            if !self.identities.insert(name.clone()) {
                return Err(error(format!(
                    "Qualified declaration identity collision: {name:?}"
                )));
            }
            insert(&mut symbols, local, name, &source.id)?;
        }
        let mut imports: Vec<_> = module.imports.iter().collect();
        imports.sort_by(|left, right| left.source.cmp(&right.source));
        for import in imports {
            if import.source.is_empty() || import.names.is_empty() {
                return Err(error(format!("Module {:?} has an empty import", source.id)));
            }
            let key = (source.id.clone(), import.source.clone());
            let imported = if let Some(imported) = self.requests.get(&key) {
                imported.clone()
            } else {
                let imported = self.resolver.resolve(&source.id, &import.source)?;
                self.requests.insert(key, imported.clone());
                imported
            };
            let exports = self.visit(imported, depth + 1)?;
            for name in &import.names {
                let target = exports.get(&name.name).ok_or_else(|| {
                    error(format!(
                        "Module {:?} does not export {:?}, imported by {:?}",
                        import.source, name.name, source.id
                    ))
                })?;
                insert(&mut symbols, &name.alias, target.clone(), &source.id)?;
            }
        }
        let mut exports = Symbols::new();
        for name in &module.exports {
            let target = symbols.get(name).ok_or_else(|| {
                error(format!("Unknown export {name:?} in module {:?}", source.id))
            })?;
            insert(&mut exports, name, target.clone(), &source.id)?;
        }
        if index == 0 {
            self.roots.extend(symbols.values().cloned());
        }
        for domain in module.domains {
            self.domains.insert(symbols[&domain.name].clone(), domain);
        }
        self.events.extend(module.events);
        for mut template in module.templates {
            super::domains::resolve(
                &mut template.branch,
                &symbols,
                &self.domains,
                &mut self.domain_bytes,
            )
            .map_err(|error| in_source(error, &source.id))?;
            super::templates::validate(&template)?;
            template
                .branch
                .name
                .clone_from(&symbols[&template.branch.name]);
            let dependencies = super::references::rewrite_template(&mut template.branch, &symbols)
                .map_err(|error| in_source(error, &source.id))?;
            self.templates
                .insert(template.branch.name.clone(), (template, dependencies));
        }
        for mut instance in module.instances {
            instance.name.clone_from(&symbols[&instance.name]);
            instance.template = symbols.get(&instance.template).cloned().ok_or_else(|| {
                error(format!(
                    "Unknown template {:?} in module {:?}",
                    instance.template, source.id
                ))
            })?;
            self.instances.push(instance);
        }
        for mut branch in module.branches {
            super::domains::resolve(&mut branch, &symbols, &self.domains, &mut self.domain_bytes)
                .map_err(|error| in_source(error, &source.id))?;
            branch.name.clone_from(&symbols[&branch.name]);
            let dependencies = super::references::rewrite(&mut branch, &symbols)
                .map_err(|error| in_source(error, &source.id))?;
            let name = branch.name.clone();
            if self
                .branches
                .insert(name.clone(), (branch, dependencies))
                .is_some()
            {
                return Err(error(format!(
                    "Qualified branch identity collision: {name:?}"
                )));
            }
        }
        self.exports.insert(source.id, exports.clone());
        Ok(exports)
    }
}

fn insert(
    symbols: &mut Symbols,
    name: &str,
    target: String,
    source: &str,
) -> Result<(), Diagnostic> {
    if name.is_empty() || symbols.insert(name.to_owned(), target).is_some() {
        return Err(error(format!(
            "Empty or duplicate name {name:?} in module {source:?}"
        )));
    }
    Ok(())
}

pub(super) fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("invalid_module", message, None)
}

fn in_source(mut error: Diagnostic, source: &str) -> Diagnostic {
    error.message = format!("Module {source:?}: {}", error.message);
    error
}

use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use smsl_core::{
    Diagnostic,
    modules::{Resolver, Source, SourceFormat},
};

#[derive(Default)]
pub struct Files {
    sources: BTreeMap<PathBuf, Source>,
}

impl Files {
    pub fn root(&mut self, path: &str) -> Result<Source, Diagnostic> {
        if path != "-" {
            return self.read(Path::new(path));
        }
        let mut text = String::new();
        io::stdin().read_to_string(&mut text).map_err(input_error)?;
        let path = std::env::current_dir()
            .map_err(input_error)?
            .join("stdin.smsl");
        Ok(Source {
            id: identity(&path)?,
            text,
            format: SourceFormat::Smsl,
        })
    }

    fn read(&mut self, path: &Path) -> Result<Source, Diagnostic> {
        let path = path
            .canonicalize()
            .map_err(|error| input_error(format!("{}: {error}", path.display())))?;
        if let Some(source) = self.sources.get(&path) {
            return Ok(source.clone());
        }
        let source = Source {
            id: identity(&path)?,
            text: fs::read_to_string(&path)
                .map_err(|error| input_error(format!("{}: {error}", path.display())))?,
            format: match path.extension().and_then(|part| part.to_str()) {
                Some("json") => SourceFormat::Json,
                Some("yaml" | "yml") => SourceFormat::Yaml,
                Some("xml") => SourceFormat::Xml,
                Some("toml") => SourceFormat::Toml,
                _ => SourceFormat::Smsl,
            },
        };
        self.sources.insert(path, source.clone());
        Ok(source)
    }
}

impl Resolver for Files {
    fn resolve(&mut self, importer: &str, source: &str) -> Result<Source, Diagnostic> {
        self.read(
            &Path::new(importer)
                .parent()
                .expect("absolute source identity")
                .join(source),
        )
    }
}

fn identity(path: &Path) -> Result<String, Diagnostic> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| input_error("Module paths must be UTF-8"))
}

fn input_error(error: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::new("invalid_source", error.to_string(), None)
}

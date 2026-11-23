use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

const USAGE: &str = "Usage: smsl fmt [--check] [PATH ... | -]";

pub fn run(args: &[&str]) -> Result<(), Box<dyn Error>> {
    let mut check = false;
    let mut positional = false;
    let mut paths = Vec::new();
    for &arg in args {
        match arg {
            "--" if !positional => positional = true,
            "--check" if !positional => check = true,
            "-h" | "--help" if !positional => {
                println!(
                    "{USAGE}\n\nFiles are formatted in place. Directories select .smsl files recursively.\nNo paths selects the current directory. - reads stdin and writes stdout.\n--check reports changes without writing files."
                );
                return Ok(());
            }
            arg if !positional && arg.starts_with('-') && arg != "-" => {
                return Err(format!("Unknown formatting option {arg:?}\n{USAGE}").into());
            }
            _ => paths.push(arg),
        }
    }
    if paths.contains(&"-") {
        if paths.len() != 1 {
            return Err("Standard input (-) cannot be combined with paths".into());
        }
        let mut source = String::new();
        io::stdin().read_to_string(&mut source)?;
        let formatted = smsl_core::source::format(&source)?;
        if check {
            if source != formatted {
                return Err("Standard input needs formatting".into());
            }
        } else {
            io::stdout().write_all(formatted.as_bytes())?;
        }
        return Ok(());
    }
    if paths.is_empty() {
        paths.push(".");
    }
    let mut files = BTreeSet::new();
    for path in paths {
        collect(Path::new(path), &mut files, true)?;
    }
    let mut changes = Vec::new();
    for path in files {
        let source =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let formatted = smsl_core::source::format(&source)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if source != formatted {
            changes.push((path, formatted));
        }
    }
    for (path, formatted) in &changes {
        if check {
            println!("would reformat {}", path.display());
        } else {
            fs::write(path, formatted).map_err(|error| format!("{}: {error}", path.display()))?;
            println!("formatted {}", path.display());
        }
    }
    if check && !changes.is_empty() {
        return Err(format!("{} file(s) need formatting", changes.len()).into());
    }
    Ok(())
}

fn collect(
    path: &Path,
    files: &mut BTreeSet<PathBuf>,
    explicit: bool,
) -> Result<(), Box<dyn Error>> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if metadata.is_symlink() {
        if explicit {
            return Err(format!("{}: symbolic links are not formatted", path.display()).into());
        }
    } else if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() && excluded(&entry.file_name().to_string_lossy()) {
                continue;
            }
            collect(&entry.path(), files, false)?;
        }
    } else if metadata.is_file()
        && path
            .extension()
            .is_some_and(|extension| extension == "smsl")
    {
        files.insert(fs::canonicalize(path)?);
    } else if explicit {
        return Err(format!("{}: expected a .smsl file or directory", path.display()).into());
    }
    Ok(())
}

fn excluded(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name,
            "target" | "node_modules" | "dist" | "build" | "vendor"
        )
}

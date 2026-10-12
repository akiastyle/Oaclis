use crate::invalid;
use std::io;
use std::path::{Component, Path, PathBuf};

pub(crate) fn validate_absolute(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path.as_os_str().as_encoded_bytes().contains(&0)
        || path.components().any(|part| part == Component::ParentDir)
    {
        return Err(invalid("Serve un percorso assoluto senza '..'"));
    }
    Ok(())
}

#[derive(Debug)]
pub struct ProjectPaths {
    source: PathBuf,
    destination: PathBuf,
}

impl ProjectPaths {
    pub fn new(source: impl Into<PathBuf>, destination: impl Into<PathBuf>) -> io::Result<Self> {
        let source = source.into();
        let destination = destination.into();
        for path in [&source, &destination] {
            validate_absolute(path)?;
            if path.parent().is_none() {
                return Err(invalid(
                    "La radice del filesystem non può essere un progetto",
                ));
            }
        }
        Ok(Self {
            source,
            destination,
        })
    }

    pub fn map(&self, source_path: impl AsRef<Path>) -> io::Result<PathBuf> {
        let path = source_path.as_ref();
        validate_absolute(path)?;
        let relative = path
            .strip_prefix(&self.source)
            .map_err(|_| invalid("Percorso esterno al progetto selezionato"))?;
        Ok(self.destination.join(relative))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_root_and_nested_files_without_prefix_collisions() {
        let paths = ProjectPaths::new("/work/app", "/home/user/app").unwrap();
        assert_eq!(paths.map("/work/app").unwrap(), Path::new("/home/user/app"));
        assert_eq!(
            paths.map("/work/app/src/main.rs").unwrap(),
            Path::new("/home/user/app/src/main.rs")
        );
        for path in ["/work/app2/file", "/work/app/../secret", "src/main.rs"] {
            assert!(paths.map(path).is_err());
        }
    }

    #[test]
    fn rejects_unsafe_project_roots() {
        for (source, destination) in [
            ("/", "/work"),
            ("/work", "/"),
            ("relative", "/work"),
            ("/work", "/target/../other"),
        ] {
            assert!(ProjectPaths::new(source, destination).is_err());
        }
    }
}

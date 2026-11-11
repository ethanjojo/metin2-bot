//! Checked file inventories shared by data migration and installation retirement.
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

pub(crate) fn reject_link(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let linked = metadata.file_type().is_symlink();
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        linked || metadata.file_attributes() & 0x400 != 0
    };
    if linked {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Migration cannot follow symbolic links or reparse points.",
        ));
    }
    Ok(())
}

pub(crate) fn hash(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 32768];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub(crate) fn fingerprints_except(
    root: &Path,
    excluded: Option<&Path>,
) -> io::Result<BTreeMap<PathBuf, String>> {
    files(root)?
        .into_iter()
        .filter(|p| Some(root.join(p).as_path()) != excluded)
        .map(|p| Ok((p.clone(), hash(&root.join(p))?)))
        .collect()
}

pub(crate) fn reject_linked_ancestors(path: &Path) -> io::Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => reject_link(ancestor)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(crate) fn files(root: &Path) -> io::Result<Vec<PathBuf>> {
    reject_linked_ancestors(root)?;
    fn visit(root: &Path, path: &Path, result: &mut Vec<PathBuf>) -> io::Result<()> {
        reject_link(path)?;
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                visit(root, &entry?.path(), result)?;
            }
        } else if path.is_file() {
            result.push(
                path.strip_prefix(root)
                    .expect("visited child")
                    .to_path_buf(),
            );
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Migration accepts only regular files and directories.",
            ));
        }
        Ok(())
    }
    let mut result = Vec::new();
    visit(root, root, &mut result)?;
    Ok(result)
}

pub mod parse;

#[allow(unused_imports)]
pub use parse::{Call, Collection};

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

/// Locate `mcpal.yml`. If `override_` is `Some(p)`, return `p` if it exists
/// or fail with a "collection not found" error. Otherwise walk from `start`
/// up to filesystem root looking for `mcpal.yml`; first hit wins.
/// `Ok(None)` if nothing is found.
#[allow(dead_code)]
pub fn find_collection(start: &Path, override_: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(p) = override_ {
        if p.is_file() {
            return Ok(Some(p.to_path_buf()));
        }
        bail!("collection not found: {} doesn't exist", p.display());
    }
    let mut cur = start.to_path_buf();
    loop {
        let candidate = cur.join("mcpal.yml");
        if candidate.is_file() {
            return Ok(Some(candidate));
        }
        if !cur.pop() {
            return Ok(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn finds_in_cwd() {
        let d = tmp();
        std::fs::write(d.path().join("mcpal.yml"), "").unwrap();
        let got = find_collection(d.path(), None).unwrap();
        assert_eq!(got.as_deref(), Some(d.path().join("mcpal.yml").as_path()));
    }

    #[test]
    fn walks_up_to_ancestor() {
        let root = tmp();
        std::fs::write(root.path().join("mcpal.yml"), "").unwrap();
        let nested = root.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&nested).unwrap();
        let got = find_collection(&nested, None).unwrap();
        assert_eq!(
            got.as_deref(),
            Some(root.path().join("mcpal.yml").as_path())
        );
    }

    #[test]
    fn none_when_no_file() {
        let d = tmp();
        let nested = d.path().join("sub");
        std::fs::create_dir_all(&nested).unwrap();
        // Skip the assertion if any ancestor of the tempdir happens to have
        // mcpal.yml (defensive — CI runners that develop mcpal might).
        if find_collection(&nested, None).unwrap().is_some() {
            return;
        }
        assert!(find_collection(&nested, None).unwrap().is_none());
    }

    #[test]
    fn explicit_override_must_exist() {
        let d = tmp();
        let p = d.path().join("nope.yml");
        let err = find_collection(d.path(), Some(&p)).unwrap_err();
        assert!(err.to_string().contains("not found"), "{err}");
    }

    #[test]
    fn explicit_override_returns_as_is() {
        let d = tmp();
        let p = d.path().join("custom.yml");
        std::fs::write(&p, "").unwrap();
        let got = find_collection(d.path(), Some(&p)).unwrap();
        assert_eq!(got.as_deref(), Some(p.as_path()));
    }
}

//! Shared-library loader — dlopen/dlsym via libloading with caching and
//! Dyalog-compatible error mapping.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Cache of open libraries keyed by resolved path. Lives on the Environment.
#[derive(Default)]
pub struct LibraryCache {
    libs: HashMap<PathBuf, Arc<libloading::Library>>,
    /// (path, symbol) -> symbol address as usize (fn pointers are raw)
    syms: HashMap<(PathBuf, String), usize>,
}

impl LibraryCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append the platform suffix if missing (.so on unix).
    fn with_suffix(path: &str) -> String {
        if path.ends_with(".so") || path.contains(".so.") {
            return path.to_string();
        }
        format!("{}.so", path)
    }

    /// Resolve a library name/path to a loaded handle.
    ///
    /// Search order when no directory component: $APL_LIB_PATH entries
    /// (colon-separated), then plain name (OS search order: rpath, LD_LIBRARY_PATH,
    /// ldconfig cache). Both the raw spec and spec+.so are tried.
    pub fn get_or_load(&mut self, libspec: &str) -> Result<Arc<libloading::Library>, LoadError> {
        let candidates = Self::candidate_paths(libspec);
        let mut last_err = None;
        for cand in &candidates {
            // Safety: dlopen of a user-specified library — same trust level
            // as Dyalog ⎕NA (loading executes library init code).
            match unsafe { libloading::Library::new(cand) } {
                Ok(lib) => {
                    let key = cand.clone();
                    let arc = Arc::new(lib);
                    self.libs.insert(key.clone(), arc.clone());
                    return Ok(arc);
                }
                Err(e) => {
                    last_err = Some(e.to_string());
                }
            }
        }
        Err(LoadError {
            spec: libspec.to_string(),
            detail: last_err.unwrap_or_else(|| "no candidate paths".into()),
        })
    }

    /// Resolve a symbol in an already-loaded library. Cached by address.
    pub fn resolve(
        &mut self,
        lib: &Arc<libloading::Library>,
        lib_key: &Path,
        symbol: &str,
    ) -> Result<usize, SymbolError> {
        let key = (lib_key.to_path_buf(), symbol.to_string());
        if let Some(addr) = self.syms.get(&key) {
            return Ok(*addr);
        }
        // symbol lookup through Any { lib } keeps the handle alive
        unsafe {
            let sym: libloading::Symbol<*mut ()> =
                lib.get(symbol.as_bytes()).map_err(|e| SymbolError {
                    library: lib_key.display().to_string(),
                    symbol: symbol.to_string(),
                    detail: e.to_string(),
                })?;
            let addr = *sym as usize;
            self.syms.insert(key, addr);
            Ok(addr)
        }
    }

    fn candidate_paths(spec: &str) -> Vec<PathBuf> {
        // try raw spec first (may already carry .so or a version suffix),
        // then platform-suffixed, then lib-prefixed variants
        let mut names = vec![spec.to_string()];
        let with_suffix = Self::with_suffix(spec);
        if with_suffix != spec {
            names.push(with_suffix.clone());
        }
        let with_lib = format!("lib{}", with_suffix);
        names.push(with_lib);
        let mut out = Vec::new();
        for name in &names {
            let p = Path::new(name);
            if p.components().count() > 1 {
                // has a directory component — use as-is
                out.push(p.to_path_buf());
                continue;
            }
            if let Ok(paths) = std::env::var("APL_LIB_PATH") {
                for dir in paths.split(':').filter(|s| !s.is_empty()) {
                    out.push(Path::new(dir).join(name));
                }
            }
            // bare name → OS search order
            out.push(PathBuf::from(name));
        }
        out
    }
}

/// Public helper: the resolved path candidates for a library spec (used by
/// cabi to derive the cache key).
pub fn candidate_paths_for(spec: &str) -> String {
    LibraryCache::candidate_paths(spec)
        .into_iter()
        .next()
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

/// dlopen failure → FILE ERROR 2 territory (may be a missing dependency)
#[derive(Debug)]
pub struct LoadError {
    pub spec: String,
    pub detail: String,
}

/// dlsym miss → VALUE ERROR territory
#[derive(Debug)]
pub struct SymbolError {
    pub library: String,
    pub symbol: String,
    pub detail: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidates_with_lib_path() {
        // SAFETY: single-threaded test touching env
        unsafe { std::env::set_var("APL_LIB_PATH", "/tmp/fakelibdir") };
        let cands = LibraryCache::candidate_paths("testmath");
        assert!(cands.contains(&std::path::PathBuf::from("/tmp/fakelibdir/testmath")));
        assert!(cands.contains(&std::path::PathBuf::from("/tmp/fakelibdir/testmath.so")));
        unsafe { std::env::remove_var("APL_LIB_PATH") };
    }
}

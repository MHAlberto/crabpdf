//! Stable source identity used by profile-adjacent stores and import caches.
use std::path::Path;

pub fn fingerprint(path: &Path) -> Option<u64> {
    let metadata = path.metadata().ok()?;
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let path = path.canonicalize().ok()?;
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in path
        .to_string_lossy()
        .bytes()
        .chain(metadata.len().to_le_bytes())
        .chain(modified.to_le_bytes())
    {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    Some(hash)
}

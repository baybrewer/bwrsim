//! Save slot store over one directory (plan C2). Plain std + serde, no Tauri
//! types, so it is unit-testable on a temp dir. The host decides what goes in
//! a save; this module owns ids, names, limits and safe file handling.

use std::collections::hash_map::RandomState;
use std::fs;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use bwr_core::api::{ModeSwitch, RunSummary, ScenarioName};
use bwr_core::sim::ENGINE_STATE_VERSION;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Most save slots. The only definition; the UI reads it from `SaveList::max`.
pub const MAX_SAVES: usize = 10;
/// Save file envelope version.
pub const SAVE_FORMAT: u32 = 1;
pub const MAX_NAME_CHARS: usize = 40;

const TMP_PREFIX: &str = ".tmp-";

/// On-disk save file (plan C2).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveFile {
    pub format: u32,
    pub engine_version: u32,
    pub id: String,
    pub name: String,
    pub saved_at_ms: u64,
    pub sim_time_s: f64,
    pub scenario: ScenarioName,
    pub run_summary: RunSummary,
    pub engine: Value,
    pub ui: Value,
}

/// Listing entry; serializes exactly as ui/src/sim/types.ts `SaveMeta`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveMeta {
    pub id: String,
    pub name: String,
    pub saved_at_ms: u64,
    pub sim_time_s: f64,
    pub scenario: ScenarioName,
    pub run_summary: RunSummary,
    pub loadable: bool,
    pub error: Option<String>,
}

/// Serializes exactly as ui/src/sim/types.ts `SaveList`.
#[derive(Clone, Debug, Serialize)]
pub struct SaveList {
    pub max: usize,
    pub saves: Vec<SaveMeta>,
}

/// What the host captured for one save; the store adds id, name and time.
pub struct SaveContent {
    pub sim_time_s: f64,
    pub scenario: ScenarioName,
    pub run_summary: RunSummary,
    pub engine: Value,
    pub ui: Value,
}

/// Random 64-bit value from std only: `RandomState` keys are random per process,
/// and the counter and clock separate calls within it.
pub fn random_u64() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut h = RandomState::new().build_hasher();
    h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    h.write_u128(SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    h.write_u32(std::process::id());
    h.finish()
}

/// `^s_[0-9a-f]{16}$`
pub fn valid_id(id: &str) -> bool {
    id.len() == 18 && id.starts_with("s_") && id[2..].bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn normalize_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    let n = name.chars().count();
    if n == 0 || n > MAX_NAME_CHARS {
        return Err(format!("name must be 1-{MAX_NAME_CHARS} characters"));
    }
    Ok(name.to_string())
}

/// Write `bytes` to `target` via a temp file in the same directory, fsync and
/// rename, so a reader never sees a partial save. `before_rename` is a test seam
/// for simulating a failure after the temp file is written.
fn write_atomic(target: &Path, bytes: &[u8], before_rename: impl FnOnce(&Path) -> io::Result<()>) -> io::Result<()> {
    let dir = target.parent().ok_or_else(|| io::Error::other("save path has no directory"))?;
    let tmp = dir.join(format!("{TMP_PREFIX}{:016x}", random_u64()));
    let result = (|| {
        let mut f = fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        before_rename(&tmp)?;
        fs::rename(&tmp, target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
        return result;
    }
    // Persist the rename itself; not supported on every platform, so best effort.
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

pub struct SaveStore {
    dir: PathBuf,
    /// Serializes every list-check-mutate sequence (limit, name uniqueness,
    /// existence) so concurrent commands cannot both pass a check.
    lock: Mutex<()>,
}

impl SaveStore {
    /// The directory is created on first write, so a missing or unwritable data
    /// directory never stops the app from starting.
    pub fn new(dir: PathBuf) -> Self {
        SaveStore { dir, lock: Mutex::new(()) }
    }

    fn guard(&self) -> MutexGuard<'_, ()> {
        // The guarded data is `()`; a panic elsewhere leaves nothing inconsistent.
        self.lock.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// `<dir>/<id>.json` for a valid id only, so no id can reach outside the dir.
    fn path_of(&self, id: &str) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err(format!("invalid save id {id:?}"));
        }
        Ok(self.dir.join(format!("{id}.json")))
    }

    /// Existing regular file (not a symlink) at `path`.
    fn check_regular(path: &Path) -> Result<(), String> {
        match fs::symlink_metadata(path) {
            Ok(m) if m.file_type().is_file() => Ok(()),
            Ok(_) => Err("save is not a regular file".into()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err("save not found".into()),
            Err(e) => Err(format!("save unreadable: {e}")),
        }
    }

    fn read_file(path: &Path) -> Result<SaveFile, String> {
        Self::check_regular(path)?;
        let bytes = fs::read(path).map_err(|e| format!("save unreadable: {e}"))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("save file invalid: {e}"))
    }

    fn meta_of(id: &str, path: &Path) -> SaveMeta {
        let file_name = format!("{id}.json");
        match Self::read_file(path) {
            Ok(f) => {
                let error = if f.format != SAVE_FORMAT {
                    Some(format!("{file_name}: save format {}, this build reads {SAVE_FORMAT}", f.format))
                } else if f.engine_version != ENGINE_STATE_VERSION {
                    Some(format!("{file_name}: engine version {}, this build reads {ENGINE_STATE_VERSION}", f.engine_version))
                } else if f.id != id {
                    Some(format!("{file_name}: id inside the file is {:?}", f.id))
                } else {
                    None
                };
                SaveMeta {
                    id: id.to_string(),
                    name: f.name,
                    saved_at_ms: f.saved_at_ms,
                    sim_time_s: f.sim_time_s,
                    scenario: f.scenario,
                    run_summary: f.run_summary,
                    loadable: error.is_none(),
                    error,
                }
            }
            // Placeholders: only `id`, `name` (the file name), `savedAtMs` (mtime),
            // `loadable` and `error` mean anything for an unreadable file.
            Err(e) => SaveMeta {
                id: id.to_string(),
                name: file_name.clone(),
                saved_at_ms: fs::symlink_metadata(path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                sim_time_s: 0.0,
                scenario: ScenarioName::ColdShutdown,
                run_summary: RunSummary { mode: ModeSwitch::Shutdown, power_pct: 0.0, pressure_mpa: 0.0, scrammed: false },
                loadable: false,
                error: Some(format!("{file_name}: {e}")),
            },
        }
    }

    /// Every `<id>.json` with a valid id; temp files and foreign names are not saves.
    fn list_unlocked(&self) -> Vec<SaveMeta> {
        let Ok(entries) = fs::read_dir(&self.dir) else { return Vec::new() };
        let mut saves: Vec<SaveMeta> = entries
            .filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let id = name.strip_suffix(".json")?;
                valid_id(id).then(|| Self::meta_of(id, &e.path()))
            })
            .collect();
        saves.sort_by(|a, b| b.saved_at_ms.cmp(&a.saved_at_ms).then_with(|| a.id.cmp(&b.id)));
        saves
    }

    /// Newest first. Never fails: problems become `loadable: false` entries.
    pub fn list(&self) -> SaveList {
        let _g = self.guard();
        SaveList { max: MAX_SAVES, saves: self.list_unlocked() }
    }

    /// Create (`id: None`) or overwrite an existing save.
    pub fn write(&self, id: Option<&str>, name: &str, content: SaveContent) -> Result<SaveMeta, String> {
        self.write_with(id, name, content, |_| Ok(()))
    }

    fn write_with(
        &self,
        id: Option<&str>,
        name: &str,
        content: SaveContent,
        before_rename: impl FnOnce(&Path) -> io::Result<()>,
    ) -> Result<SaveMeta, String> {
        let name = normalize_name(name)?;
        if !content.ui.is_object() {
            return Err("run record must be a JSON object".into());
        }
        let _g = self.guard();
        let existing = self.list_unlocked();
        let id = match id {
            Some(id) => {
                let path = self.path_of(id)?;
                Self::check_regular(&path)?;
                id.to_string()
            }
            None => {
                if existing.len() >= MAX_SAVES {
                    return Err(format!("all {MAX_SAVES} save slots are in use"));
                }
                loop {
                    let id = format!("s_{:016x}", random_u64());
                    if !existing.iter().any(|m| m.id == id) && !self.path_of(&id)?.exists() {
                        break id;
                    }
                }
            }
        };
        let lower = name.to_lowercase();
        // Unreadable entries carry no real name, so they cannot clash.
        if existing.iter().any(|m| m.id != id && m.error.is_none() && m.name.to_lowercase() == lower) {
            return Err(format!("a save named {name:?} already exists"));
        }
        let file = SaveFile {
            format: SAVE_FORMAT,
            engine_version: ENGINE_STATE_VERSION,
            id: id.clone(),
            name,
            saved_at_ms: now_ms(),
            sim_time_s: content.sim_time_s,
            scenario: content.scenario,
            run_summary: content.run_summary,
            engine: content.engine,
            ui: content.ui,
        };
        let bytes = serde_json::to_vec(&file).map_err(|e| format!("save encode failed: {e}"))?;
        fs::create_dir_all(&self.dir).map_err(|e| format!("save directory unavailable: {e}"))?;
        let path = self.path_of(&id)?;
        write_atomic(&path, &bytes, before_rename).map_err(|e| format!("save write failed: {e}"))?;
        Ok(Self::meta_of(&id, &path))
    }

    /// The parsed file, with format and engine version checked.
    pub fn read(&self, id: &str) -> Result<SaveFile, String> {
        let path = self.path_of(id)?;
        let _g = self.guard();
        let f = Self::read_file(&path)?;
        if f.format != SAVE_FORMAT {
            return Err(format!("save format {}, this build reads {SAVE_FORMAT}", f.format));
        }
        if f.engine_version != ENGINE_STATE_VERSION {
            return Err(format!("engine version {}, this build reads {ENGINE_STATE_VERSION}", f.engine_version));
        }
        Ok(f)
    }

    /// Removes a regular file or a planted symlink (unlinking never follows it,
    /// and a listed entry must be removable to free its slot).
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let path = self.path_of(id)?;
        let _g = self.guard();
        match fs::symlink_metadata(&path) {
            Ok(m) if m.file_type().is_file() || m.file_type().is_symlink() => {
                fs::remove_file(&path).map_err(|e| format!("delete failed: {e}"))
            }
            Ok(_) => Err("save is not a regular file".into()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err("save not found".into()),
            Err(e) => Err(format!("delete failed: {e}")),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    /// Unique temp dir, removed on drop.
    pub(crate) struct TempDir(pub PathBuf);
    impl TempDir {
        pub(crate) fn new() -> Self {
            let p = std::env::temp_dir().join(format!("bwrsim-saves-{:016x}", random_u64()));
            fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn content() -> SaveContent {
        SaveContent {
            sim_time_s: 12.5,
            scenario: ScenarioName::HotStandby,
            run_summary: RunSummary { mode: ModeSwitch::Startup, power_pct: 1.0, pressure_mpa: 7.0, scrammed: false },
            engine: serde_json::json!({ "version": ENGINE_STATE_VERSION, "sim": {} }),
            ui: serde_json::json!({ "v": 1 }),
        }
    }

    fn fill(store: &SaveStore, n: usize) {
        for i in 0..n {
            store.write(None, &format!("save {i}"), content()).unwrap();
        }
    }

    fn dir_names(dir: &Path) -> Vec<String> {
        fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect()
    }

    #[test]
    fn id_grammar() {
        assert!(valid_id("s_0123456789abcdef"));
        for bad in ["s_0123456789ABCDEF", "s_0123456789abcde", "s_0123456789abcdef0", "../x", "/etc/passwd", "x_0123456789abcdef", "s_../../../../x"] {
            assert!(!valid_id(bad), "{bad}");
        }
        assert!(valid_id(&format!("s_{:016x}", random_u64())));
    }

    #[test]
    fn meta_serializes_like_the_ui_type() {
        let dir = TempDir::new();
        let store = SaveStore::new(dir.0.clone());
        let meta = store.write(None, "  Startup  ", content()).unwrap();
        let v = serde_json::to_value(&meta).unwrap();
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        let mut keys = keys;
        keys.sort();
        assert_eq!(keys, ["error", "id", "loadable", "name", "runSummary", "savedAtMs", "scenario", "simTimeS"]);
        assert_eq!(v["error"], Value::Null);
        assert_eq!(v["name"], "Startup");
        assert_eq!(v["scenario"], "hotStandby");
        assert_eq!(v["runSummary"]["pressureMpa"], 7.0);
        let list = serde_json::to_value(store.list()).unwrap();
        assert_eq!(list["max"], MAX_SAVES);
        assert_eq!(list["saves"][0]["id"], meta.id.as_str());
    }

    #[test]
    fn ten_slot_limit_and_newest_first() {
        let dir = TempDir::new();
        let store = SaveStore::new(dir.0.clone());
        fill(&store, MAX_SAVES);
        assert!(store.write(None, "eleven", content()).unwrap_err().contains("slots"));
        let list = store.list();
        assert_eq!(list.saves.len(), MAX_SAVES);
        assert!(list.saves.windows(2).all(|w| w[0].saved_at_ms >= w[1].saved_at_ms));
        // Overwriting at the limit is still allowed.
        let id = list.saves[3].id.clone();
        store.write(Some(&id), "renamed", content()).unwrap();
        assert_eq!(store.list().saves[0].id, id);
    }

    #[test]
    fn concurrent_creates_at_nine_slots_admit_exactly_one() {
        let dir = TempDir::new();
        let store = Arc::new(SaveStore::new(dir.0.clone()));
        fill(&store, MAX_SAVES - 1);
        let n = 8;
        let barrier = Arc::new(Barrier::new(n));
        let handles: Vec<_> = (0..n)
            .map(|i| {
                let (store, barrier) = (store.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    store.write(None, &format!("racer {i}"), content()).is_ok()
                })
            })
            .collect();
        let ok = handles.into_iter().map(|h| h.join().unwrap()).filter(|&ok| ok).count();
        assert_eq!(ok, 1);
        assert_eq!(store.list().saves.len(), MAX_SAVES);
    }

    #[test]
    fn same_name_race_admits_one() {
        let dir = TempDir::new();
        let store = Arc::new(SaveStore::new(dir.0.clone()));
        let barrier = Arc::new(Barrier::new(4));
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let (store, barrier) = (store.clone(), barrier.clone());
                // Case and padding differ; uniqueness is on the trimmed, case-folded name.
                let name = if i % 2 == 0 { "Hot Start" } else { " hot start " };
                std::thread::spawn(move || {
                    barrier.wait();
                    store.write(None, name, content()).is_ok()
                })
            })
            .collect();
        let ok = handles.into_iter().map(|h| h.join().unwrap()).filter(|&ok| ok).count();
        assert_eq!(ok, 1);
        assert_eq!(store.list().saves.len(), 1);
    }

    #[test]
    fn name_rules() {
        let dir = TempDir::new();
        let store = SaveStore::new(dir.0.clone());
        assert!(store.write(None, "   ", content()).is_err());
        assert!(store.write(None, &"x".repeat(41), content()).is_err());
        let a = store.write(None, &"é".repeat(40), content()).unwrap();
        let b = store.write(None, "B", content()).unwrap();
        // Keeping its own name on overwrite is fine; taking another's is not.
        store.write(Some(&b.id), "b", content()).unwrap();
        assert!(store.write(Some(&b.id), &"É".repeat(40), content()).is_err());
        assert_eq!(store.list().saves.iter().find(|m| m.id == a.id).unwrap().name, "é".repeat(40));
    }

    #[test]
    fn overwrite_of_unknown_id_is_rejected_not_created() {
        let dir = TempDir::new();
        let store = SaveStore::new(dir.0.clone());
        let err = store.write(Some("s_0000000000000000"), "x", content()).unwrap_err();
        assert!(err.contains("not found"), "{err}");
        assert!(store.list().saves.is_empty());
    }

    #[test]
    fn hostile_ids_are_rejected() {
        let dir = TempDir::new();
        let store = SaveStore::new(dir.join_safe());
        let outside = dir.0.join("outside.json");
        fs::write(&outside, b"{}").unwrap();
        let abs = outside.to_string_lossy().into_owned();
        for id in ["../x", "../outside", abs.as_str(), "s_0123456789abcdef/../../outside"] {
            assert!(store.read(id).is_err(), "{id}");
            assert!(store.delete(id).is_err(), "{id}");
            assert!(store.write(Some(id), "x", content()).is_err(), "{id}");
        }
        assert!(outside.exists());
    }

    impl TempDir {
        /// A store dir nested inside the temp dir, so "outside" files are still cleaned up.
        fn join_safe(&self) -> PathBuf {
            let p = self.0.join("saves");
            fs::create_dir_all(&p).unwrap();
            p
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_save_is_rejected_for_read_and_overwrite() {
        let dir = TempDir::new();
        let saves = dir.join_safe();
        let store = SaveStore::new(saves.clone());
        let real = store.write(None, "real", content()).unwrap();
        let target = dir.0.join("target.json");
        fs::copy(saves.join(format!("{}.json", real.id)), &target).unwrap();
        let id = "s_00000000000000aa";
        std::os::unix::fs::symlink(&target, saves.join(format!("{id}.json"))).unwrap();
        assert!(store.read(id).unwrap_err().contains("regular"));
        assert!(store.write(Some(id), "evil", content()).is_err());
        let listed = store.list().saves.into_iter().find(|m| m.id == id).unwrap();
        assert!(!listed.loadable);
        // Delete unlinks the link only.
        store.delete(id).unwrap();
        assert!(target.exists());
    }

    #[test]
    fn temp_and_foreign_files_are_not_listed() {
        let dir = TempDir::new();
        let store = SaveStore::new(dir.0.clone());
        fill(&store, 1);
        fs::write(dir.0.join(".tmp-0123456789abcdef"), b"partial").unwrap();
        fs::write(dir.0.join("notes.json"), b"{}").unwrap();
        fs::write(dir.0.join("s_0123456789abcdef.json.bak"), b"{}").unwrap();
        assert_eq!(store.list().saves.len(), 1);
    }

    #[test]
    fn unreadable_file_is_listed_not_fatal_and_counts_toward_the_limit() {
        let dir = TempDir::new();
        let store = SaveStore::new(dir.0.clone());
        fill(&store, MAX_SAVES - 1);
        let id = "s_00000000000000bb";
        fs::write(dir.0.join(format!("{id}.json")), b"{ not json").unwrap();
        let list = store.list();
        assert_eq!(list.saves.len(), MAX_SAVES);
        let bad = list.saves.iter().find(|m| m.id == id).unwrap();
        assert!(!bad.loadable);
        assert!(bad.error.as_deref().unwrap().contains(&format!("{id}.json")));
        assert!(store.read(id).is_err());
        assert!(store.write(None, "full", content()).is_err());
        store.delete(id).unwrap();
        store.write(None, "full", content()).unwrap();
    }

    #[test]
    fn incompatible_engine_version_is_listed_not_loadable() {
        let dir = TempDir::new();
        let store = SaveStore::new(dir.0.clone());
        let meta = store.write(None, "old", content()).unwrap();
        let path = dir.0.join(format!("{}.json", meta.id));
        let mut v: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        v["engineVersion"] = (ENGINE_STATE_VERSION + 1).into();
        fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
        let listed = store.list().saves.pop().unwrap();
        assert!(!listed.loadable);
        assert_eq!(listed.name, "old");
        assert!(listed.error.unwrap().contains("engine version"));
        assert!(store.read(&meta.id).is_err());
    }

    #[test]
    fn failed_write_leaves_no_partial_or_temp_file() {
        let dir = TempDir::new();
        let store = SaveStore::new(dir.0.clone());
        let meta = store.write(None, "keep", content()).unwrap();
        let path = dir.0.join(format!("{}.json", meta.id));
        let before = fs::read(&path).unwrap();
        let fail = |_: &Path| Err(io::Error::other("disk full"));
        assert!(store.write_with(Some(&meta.id), "changed", content(), fail).is_err());
        assert!(store.write_with(None, "new", content(), fail).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(dir_names(&dir.0), [format!("{}.json", meta.id)]);
    }

    #[test]
    fn round_trip_and_delete() {
        let dir = TempDir::new();
        let store = SaveStore::new(dir.0.join("nested/saves"));
        assert!(store.list().saves.is_empty());
        let meta = store.write(None, "one", content()).unwrap();
        let f = store.read(&meta.id).unwrap();
        assert_eq!((f.format, f.id.as_str(), f.sim_time_s), (SAVE_FORMAT, meta.id.as_str(), 12.5));
        assert_eq!(f.ui, serde_json::json!({ "v": 1 }));
        store.delete(&meta.id).unwrap();
        assert!(store.delete(&meta.id).is_err());
        assert!(store.list().saves.is_empty());
    }
}

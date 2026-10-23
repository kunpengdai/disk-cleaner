//! 扫描器：多线程目录遍历（M1）。读取 MFT 的极速扫描放在 M2。

use crate::rules::Rules;
use crate::tree::*;
use parking_lot::Mutex;
use rayon::prelude::*;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

const ATTR_DIRECTORY: u32 = 0x10;
const ATTR_REPARSE: u32 = 0x400;
const ATTR_OFFLINE: u32 = 0x1000;
const ATTR_RECALL_ON_OPEN: u32 = 0x40000;
const ATTR_RECALL_ON_DATA_ACCESS: u32 = 0x400000;

#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    pub current: Mutex<String>,
}

pub struct Ctx<'a> {
    pub progress: &'a Progress,
    pub cancel: &'a AtomicBool,
}

fn scan_dir(path: &Path, name: Box<str>, attrs: u32, mtime: u64, ctx: &Ctx) -> RawDir {
    let mut dir = RawDir { name, mtime, attrs, flags: 0, files: Vec::new(), dirs: Vec::new() };
    if ctx.cancel.load(Relaxed) {
        return dir;
    }
    ctx.progress.dirs.fetch_add(1, Relaxed);
    if let Some(mut cur) = ctx.progress.current.try_lock() {
        cur.clear();
        cur.push_str(&path.to_string_lossy());
    }
    let rd = match std::fs::read_dir(path) {
        Ok(r) => r,
        Err(_) => {
            dir.flags |= F_INACCESSIBLE;
            ctx.progress.errors.fetch_add(1, Relaxed);
            return dir;
        }
    };
    let mut subdirs: Vec<(PathBuf, Box<str>, u32, u64)> = Vec::new();
    let (mut nfiles, mut nbytes) = (0u64, 0u64);
    for ent in rd {
        let Ok(ent) = ent else {
            ctx.progress.errors.fetch_add(1, Relaxed);
            continue;
        };
        // Windows 上 DirEntry::metadata 直接用目录枚举时拿到的数据，不会再打开文件
        let Ok(md) = ent.metadata() else { continue };
        let a = md.file_attributes();
        let mt = md.last_write_time();
        let nm: Box<str> = ent.file_name().to_string_lossy().into();
        if a & ATTR_DIRECTORY != 0 {
            if a & ATTR_REPARSE != 0 {
                // 联接点 / 符号链接 / 云端目录：不进入
                dir.dirs.push(RawDir { name: nm, mtime: mt, attrs: a, flags: F_REPARSE, files: Vec::new(), dirs: Vec::new() });
            } else {
                subdirs.push((ent.path(), nm, a, mt));
            }
        } else {
            let cloud = a & (ATTR_OFFLINE | ATTR_RECALL_ON_DATA_ACCESS | ATTR_RECALL_ON_OPEN) != 0;
            let size = if cloud { 0 } else { md.len() };
            let mut flags = 0;
            if cloud {
                flags |= F_CLOUD;
            }
            if a & ATTR_REPARSE != 0 {
                flags |= F_REPARSE;
            }
            nfiles += 1;
            nbytes += size;
            dir.files.push(RawFile { name: nm, size, mtime: mt, attrs: a, flags });
        }
    }
    ctx.progress.files.fetch_add(nfiles, Relaxed);
    ctx.progress.bytes.fetch_add(nbytes, Relaxed);
    let mut scanned: Vec<RawDir> =
        subdirs.into_par_iter().map(|(p, n, a, m)| scan_dir(&p, n, a, m, ctx)).collect();
    dir.dirs.append(&mut scanned);
    dir
}

fn pool() -> rayon::ThreadPool {
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(4, 16);
    rayon::ThreadPoolBuilder::new().num_threads(n).build().expect("线程池创建失败")
}

/// 全盘扫描一个分区（或任意文件夹）
pub fn scan_full(root: &Path, rules: &Rules, ctx: &Ctx) -> Tree {
    let (attrs, mtime) = std::fs::metadata(root).map(|m| (m.file_attributes(), m.last_write_time())).unwrap_or((0, 0));
    let name: Box<str> = root.to_string_lossy().into();
    let raw = pool().install(|| scan_dir(root, name, attrs, mtime, ctx));
    let drive = components(root).into_iter().next().unwrap_or_default();
    let mut tree = Tree::new(ScanMode::Full, &drive);
    if is_drive_root(root) {
        tree.push_raw_dir(NO_PARENT, raw);
    } else {
        // 扫描的是普通文件夹：补出上级路径，保证规则能按完整路径匹配
        let v = tree.push_virtual_root("扫描结果");
        let comps = components(root);
        let parent = tree.ensure_dir(v, &comps[..comps.len().saturating_sub(1)]);
        let mut raw = raw;
        raw.name = comps.last().cloned().unwrap_or_default().into();
        tree.push_raw_dir(parent, raw);
    }
    tree.finalize(rules);
    tree
}

/// 快速扫描某个磁盘：只扫规则库里登记的可清理位置
pub fn scan_quick(rules: &Rules, drive: &str, ctx: &Ctx) -> Tree {
    if let Some(mut cur) = ctx.progress.current.try_lock() {
        *cur = format!("正在 {drive} 中查找可清理的位置…");
    }
    let targets = rules.quick_targets(drive);
    enum Got {
        Dir(RawDir),
        File(RawFile),
    }
    let results: Vec<(PathBuf, Got)> = pool().install(|| {
        targets
            .into_par_iter()
            .filter_map(|p| {
                let md = std::fs::symlink_metadata(&p).ok()?;
                let a = md.file_attributes();
                let nm: Box<str> = p.file_name()?.to_string_lossy().into();
                let got = if md.is_dir() && a & ATTR_REPARSE == 0 {
                    Got::Dir(scan_dir(&p, nm, a, md.last_write_time(), ctx))
                } else if md.is_file() {
                    ctx.progress.files.fetch_add(1, Relaxed);
                    ctx.progress.bytes.fetch_add(md.len(), Relaxed);
                    Got::File(RawFile { name: nm, size: md.len(), mtime: md.last_write_time(), attrs: a, flags: 0 })
                } else {
                    return None;
                };
                Some((p, got))
            })
            .collect()
    });
    let mut tree = Tree::new(ScanMode::Quick, &format!("{}\\", drive.trim_end_matches('\\')));
    let v = tree.push_virtual_root(&format!("快速扫描 {}", drive.trim_end_matches('\\')));
    for (p, got) in results {
        let comps = components(&p);
        let parent = tree.ensure_dir(v, &comps[..comps.len() - 1]);
        match got {
            Got::Dir(d) => tree.push_raw_dir(parent, d),
            Got::File(f) => tree.push_raw_file(parent, f),
        };
    }
    tree.finalize(rules);
    tree
}

fn is_drive_root(p: &Path) -> bool {
    let s = p.to_string_lossy();
    s.len() <= 3 && s.as_bytes().get(1) == Some(&b':')
}

/// "C:\Users\a" → ["C:\", "Users", "a"]
fn components(p: &Path) -> Vec<String> {
    let s = p.to_string_lossy();
    let mut parts = s.split('\\').filter(|x| !x.is_empty());
    let mut out = Vec::new();
    if let Some(drive) = parts.next() {
        out.push(format!("{drive}\\"));
    }
    out.extend(parts.map(String::from));
    out
}

//! 规则库：内置保护名单（代码里写死）+ 启发式条目 + builtin.json 里的清理规则。
//! 所有条目放在同一张表里，树节点只记录条目编号。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// 删除这个文件/文件夹本身
    #[serde(rename = "self")]
    Whole,
    /// 只删除文件夹里面的内容，保留文件夹
    Contents,
    /// 调用系统接口清空回收站
    EmptyRecycleBin,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Protected,
    Rule,
    Heuristic,
}

#[derive(Debug, Serialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub category: String,
    pub level: u8,
    pub mode: Mode,
    pub note: String,
    pub alt: Option<String>,
    pub origin: Origin,
    /// 受保护条目：子项是否继承同样的保护
    #[serde(skip)]
    pub inherit: bool,
    /// 是否参与快速扫描
    #[serde(skip)]
    pub quick: bool,
    /// 在这个条目管辖的范围内，是否允许 `**\xxx` 这类全局规则生效
    #[serde(skip)]
    pub allow_global: bool,
    #[serde(skip)]
    pub dir_only: bool,
}

pub enum Seg {
    Lit { raw: String, lower: String },
    Glob { lower: String },
}

impl Seg {
    pub fn matches(&self, name_lower: &str) -> bool {
        match self {
            Seg::Lit { lower, .. } => lower == name_lower,
            Seg::Glob { lower } => glob_match(lower, name_lower),
        }
    }
}

pub struct Pattern {
    pub entry: u16,
    pub segs: Vec<Seg>,
    /// `**\name`：只按名称匹配，不管在哪一层
    pub global: bool,
}

pub struct Rules {
    pub entries: Vec<Entry>,
    pub patterns: Vec<Pattern>,
    /// 从盘符开始匹配的规则编号（非全局）
    pub root_cands: Vec<u16>,
    pub globals: Vec<u16>,
}

// 启发式条目的固定编号
pub const H_DEFAULT: u16 = 0;
pub const H_DRIVE_ROOT: u16 = 1;
pub const H_EXT_TEMP: u16 = 2;
pub const H_REPARSE: u16 = 3;
pub const H_LEGACY_JUNCTION: u16 = 4;
pub const H_VIRTUAL: u16 = 5;

#[derive(Deserialize)]
struct JsonRule {
    id: String,
    name: String,
    category: String,
    paths: Vec<String>,
    level: u8,
    mode: Mode,
    note: String,
    alt: Option<String>,
    quick: Option<bool>,
    #[serde(default)]
    dir_only: bool,
}

/// 内置保护名单：(编号, 名称, 路径, 子项是否继承, 说明, 替代做法)
const PROTECTED: &[(&str, &str, &[&str], bool, &str, Option<&str>)] = &[
    ("winsxs", "Windows 组件存储", &["%SystemRoot%\\WinSxS"], true,
     "Windows 组件存储，里面大量文件是系统文件的硬链接，实际占用比显示的小。直接删除会导致系统无法更新甚至无法启动。",
     Some("以管理员身份打开命令行，运行：DISM /Online /Cleanup-Image /StartComponentCleanup")),
    ("system32", "系统核心文件", &["%SystemRoot%\\System32", "%SystemRoot%\\SysWOW64"], true,
     "Windows 核心系统文件，删除会导致系统损坏。", None),
    ("windows", "Windows 系统目录", &["%SystemRoot%"], true,
     "Windows 系统目录。除了已识别的缓存和临时文件，这里的内容都不能直接删除。",
     Some("可以使用系统自带的“磁盘清理”或 设置 → 系统 → 存储 → 临时文件。")),
    ("pagefile", "虚拟内存文件", &["*:\\pagefile.sys", "*:\\swapfile.sys"], true,
     "系统的虚拟内存文件，正在使用中，无法也不应直接删除。",
     Some("可在 设置 → 系统 → 关于 → 高级系统设置 → 性能 → 高级 → 虚拟内存 中调整大小。")),
    ("hiberfil", "休眠文件", &["*:\\hiberfil.sys"], true,
     "系统休眠和快速启动使用的文件，无法直接删除。",
     Some("如果不需要休眠功能，以管理员身份运行：powercfg /h off，文件会自动消失。")),
    ("sysvol", "系统还原点", &["*:\\System Volume Information"], true,
     "系统还原点和卷影副本，由系统管理。",
     Some("在 控制面板 → 系统 → 系统保护 → 配置 中可以删除还原点或限制占用空间。")),
    ("recycle-inner", "回收站内部", &["*:\\$Recycle.Bin\\*"], true,
     "回收站的内部结构。",
     Some("请选中上一级的 $Recycle.Bin 来清空回收站。")),
    ("boot", "系统启动文件", &["*:\\Boot", "*:\\bootmgr", "*:\\BOOTNXT", "*:\\Recovery", "*:\\$WinREAgent", "*:\\$SysReset", "*:\\$Windows.~BT", "*:\\$Windows.~WS", "*:\\$GetCurrent", "*:\\DumpStack.log*", "*:\\Config.Msi", "*:\\Documents and Settings"], true,
     "系统启动、恢复或升级相关的文件，删除可能导致系统无法启动或无法恢复。",
     Some("升级残留可以用系统自带的“磁盘清理”删除。")),
    ("windowsapps", "应用商店应用", &["*:\\Program Files\\WindowsApps", "*:\\WindowsApps", "*:\\WpSystem"], true,
     "Microsoft Store 安装的应用，受系统保护。",
     Some("在 设置 → 应用 → 已安装的应用 中卸载。")),
    ("ntuser", "用户注册表", &["%USERPROFILE%\\NTUSER*", "%USERPROFILE%\\ntuser*"], true,
     "当前用户的注册表文件，删除会导致账户无法登录。", None),
    ("profile-root", "用户文件夹", &["%SystemDrive%\\Users", "%SystemDrive%\\Users\\*"], false,
     "用户配置文件的根目录。整体删除会导致账户损坏，请只清理其中具体的子文件夹。", None),
];

const HEURISTICS: &[(&str, &str, u8, &str, Option<&str>)] = &[
    ("default", "未识别", 2,
     "没有规则能识别这个位置，可能是软件数据或个人文件。为了安全，按“谨慎”处理。", None),
    ("drive-root", "磁盘根目录", 3, "不能删除整个磁盘分区。", Some("请展开后选择具体的文件夹。")),
    ("ext-temp", "临时/日志/备份文件", 1, "从扩展名看是临时文件、日志或备份文件，通常可以删除。", None),
    ("reparse", "链接", 2,
     "这是一个链接（联接点或符号链接），指向其他位置。扫描时没有进入，也没有统计它指向的内容。删除只会删除链接本身。", None),
    ("legacy-junction", "系统兼容链接", 3,
     "Windows 为兼容旧程序创建的隐藏链接，删除可能导致旧程序出错。", None),
    ("virtual", "扫描结果", 3, "", None),
];

const TEMP_EXTS: &[&str] = &["tmp", "temp", "bak", "old", "dmp", "log", "etl", "chk", "~tmp"];

pub fn is_temp_ext(name_lower: &str) -> bool {
    match name_lower.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => TEMP_EXTS.contains(&ext),
        _ => false,
    }
}

/// 只支持 `*` 通配
pub fn glob_match(pat: &str, s: &str) -> bool {
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 {
        return pat == s;
    }
    let mut rest = s;
    let first = parts[0];
    if !rest.starts_with(first) {
        return false;
    }
    rest = &rest[first.len()..];
    let last = parts[parts.len() - 1];
    for mid in &parts[1..parts.len() - 1] {
        match rest.find(mid) {
            Some(i) => rest = &rest[i + mid.len()..],
            None => return false,
        }
    }
    rest.len() >= last.len() && rest.ends_with(last)
}

/// 展开 %VAR% 和 {known:Documents}；有变量不存在时返回 None
fn expand_env(p: &str) -> Option<String> {
    if let Some(name) = p.strip_prefix("{known:").and_then(|s| s.strip_suffix('}')) {
        return crate::winutil::known_folder(name);
    }
    let mut out = String::new();
    let mut rest = p;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after.find('%')?;
        let var = &after[..end];
        let val = std::env::var(var).ok()?;
        out.push_str(val.trim_end_matches('\\'));
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

fn compile_segs(path: &str) -> Vec<Seg> {
    path.split('\\')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let lower = s.to_lowercase();
            if s.contains('*') {
                Seg::Glob { lower }
            } else {
                Seg::Lit { raw: s.to_string(), lower }
            }
        })
        .collect()
}

impl Rules {
    pub fn load() -> Rules {
        let mut entries = Vec::new();
        let mut patterns = Vec::new();

        for (id, name, level, note, alt) in HEURISTICS {
            entries.push(Entry {
                id: id.to_string(),
                name: name.to_string(),
                category: "内置判断".into(),
                level: *level,
                mode: Mode::Whole,
                note: note.to_string(),
                alt: alt.map(String::from),
                origin: Origin::Heuristic,
                inherit: false,
                quick: false,
                allow_global: *id == "default",
                dir_only: false,
            });
        }

        let add = |entries: &mut Vec<Entry>, patterns: &mut Vec<Pattern>, e: Entry, paths: &[String]| {
            let idx = entries.len() as u16;
            entries.push(e);
            for p in paths {
                if let Some(rest) = p.strip_prefix("**\\") {
                    patterns.push(Pattern { entry: idx, segs: compile_segs(rest), global: true });
                } else if let Some(full) = expand_env(p) {
                    patterns.push(Pattern { entry: idx, segs: compile_segs(&full), global: false });
                }
            }
        };

        for (id, name, paths, inherit, note, alt) in PROTECTED {
            let paths: Vec<String> = paths.iter().map(|s| s.to_string()).collect();
            add(&mut entries, &mut patterns, Entry {
                id: id.to_string(),
                name: name.to_string(),
                category: "系统保护".into(),
                level: 3,
                mode: Mode::Whole,
                note: note.to_string(),
                alt: alt.map(String::from),
                origin: Origin::Protected,
                inherit: *inherit,
                quick: false,
                allow_global: false,
                dir_only: false,
            }, &paths);
        }

        let json: Vec<JsonRule> =
            serde_json::from_str(include_str!("../rules/builtin.json")).expect("builtin.json 格式错误");
        for r in json {
            let allow_global = matches!(r.id.as_str(), "downloads" | "personal" | "onedrive");
            add(&mut entries, &mut patterns, Entry {
                quick: r.quick.unwrap_or(r.level <= 1),
                id: r.id,
                name: r.name,
                category: r.category,
                level: r.level.min(3),
                mode: r.mode,
                note: r.note,
                alt: r.alt,
                origin: Origin::Rule,
                inherit: false,
                allow_global,
                dir_only: r.dir_only,
            }, &r.paths);
        }

        let root_cands = (0..patterns.len() as u16).filter(|&i| !patterns[i as usize].global).collect();
        let globals = (0..patterns.len() as u16).filter(|&i| patterns[i as usize].global).collect();
        Rules { entries, patterns, root_cands, globals }
    }

    pub fn entry(&self, id: u16) -> &Entry {
        &self.entries[id as usize]
    }

    /// 快速扫描某个磁盘时要扫的具体位置（已存在的路径），去掉互相嵌套的。
    /// 1. 固定路径的规则（盘符匹配，或写成 `*:` 的任意盘）
    /// 2. `**\xxx` 规则：在磁盘的前几层文件夹里按名称查找
    pub fn quick_targets(&self, drive: &str) -> Vec<PathBuf> {
        let drive = drive.trim_end_matches('\\');
        let drive_l = drive.to_lowercase();
        let drive_root = PathBuf::from(format!("{drive}\\"));
        let quick = |pat: &Pattern| {
            let e = self.entry(pat.entry);
            e.origin == Origin::Rule && e.quick && !pat.segs.is_empty()
        };
        let mut out: Vec<PathBuf> = Vec::new();
        for pat in self.patterns.iter().filter(|p| !p.global && quick(p)) {
            if pat.segs[0].matches(&drive_l) {
                out.extend(expand_segs(vec![drive_root.clone()], &pat.segs[1..]));
            }
        }
        let globals: Vec<&Pattern> = self.patterns.iter().filter(|p| p.global && quick(p)).collect();
        let mut found = Vec::new();
        shallow_find(&drive_root, 1, &globals, &mut found);
        for (p, pat) in found {
            out.extend(expand_segs(vec![p], &pat.segs[1..]));
        }
        // 去重、去嵌套
        let mut keyed: Vec<(String, PathBuf)> =
            out.into_iter().map(|p| (p.to_string_lossy().to_lowercase(), p)).collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        let mut kept: Vec<(String, PathBuf)> = Vec::new();
        for (k, p) in keyed {
            if let Some((last, _)) = kept.last() {
                if k == *last || k.starts_with(&format!("{last}\\")) {
                    continue;
                }
            }
            kept.push((k, p));
        }
        kept.into_iter().map(|(_, p)| p).collect()
    }
}

/// 从给定的起点按剩余的路径段展开成实际存在的路径
fn expand_segs(mut cur: Vec<PathBuf>, segs: &[Seg]) -> Vec<PathBuf> {
    for seg in segs {
        let mut next = Vec::new();
        for p in &cur {
            match seg {
                Seg::Lit { raw, .. } => {
                    let q = p.join(raw);
                    if std::fs::symlink_metadata(&q).is_ok() {
                        next.push(q);
                    }
                }
                Seg::Glob { lower } => {
                    if let Ok(rd) = std::fs::read_dir(p) {
                        for ent in rd.flatten() {
                            if glob_match(lower, &ent.file_name().to_string_lossy().to_lowercase()) {
                                next.push(ent.path());
                            }
                        }
                    }
                }
            }
        }
        cur = next;
        if cur.is_empty() {
            break;
        }
    }
    cur
}

/// 快速扫描按名称查找的最大层数（盘符下第一层算 1）
const SEARCH_DEPTH: usize = 5;
/// 按名称查找时不进入的文件夹：系统目录、软件目录，以及已经由固定规则覆盖的位置
const SEARCH_SKIP: &[&str] = &[
    "windows", "program files", "program files (x86)", "programdata", "appdata", "windowsapps",
    "wpsystem", "system volume information", "recovery", ".git", ".venv", "venv", "site-packages",
];

fn shallow_find<'a>(dir: &Path, depth: usize, globals: &[&'a Pattern], found: &mut Vec<(PathBuf, &'a Pattern)>) {
    if depth > SEARCH_DEPTH {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut subdirs = Vec::new();
    for ent in rd.flatten() {
        let Ok(md) = ent.metadata() else { continue };
        use std::os::windows::fs::MetadataExt;
        let a = md.file_attributes();
        if a & 0x10 == 0 || a & 0x400 != 0 {
            continue; // 不是文件夹，或是链接
        }
        let name_l = ent.file_name().to_string_lossy().to_lowercase();
        if name_l.starts_with('$') {
            continue;
        }
        let hits: Vec<&Pattern> = globals.iter().copied().filter(|g| g.segs[0].matches(&name_l)).collect();
        if !hits.is_empty() {
            for g in hits {
                found.push((ent.path(), g));
            }
        } else if !SEARCH_SKIP.contains(&name_l.as_str()) {
            subdirs.push(ent.path());
        }
    }
    for s in subdirs {
        shallow_find(&s, depth + 1, globals, found);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob() {
        assert!(glob_match("thumbcache_*.db", "thumbcache_1024.db"));
        assert!(!glob_match("thumbcache_*.db", "iconcache_1024.db"));
        assert!(glob_match("*:", "c:"));
        assert!(glob_match("ntuser*", "ntuser.dat.log1"));
        assert!(glob_match("onedrive - *", "onedrive - 公司"));
        assert!(!glob_match("a*b*c", "acb"));
    }

    #[test]
    fn temp_ext() {
        assert!(is_temp_ext("setup.log"));
        assert!(!is_temp_ext(".log"));
        assert!(!is_temp_ext("report.docx"));
    }

    #[test]
    fn loads() {
        let r = Rules::load();
        assert!(r.entries.len() > 40);
        assert!(!r.globals.is_empty());
    }
}

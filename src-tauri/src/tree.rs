//! 扫描结果树：所有节点放在一个数组里，父节点编号一定小于子节点编号。

use crate::rules::*;
use serde::Serialize;
use std::path::PathBuf;

pub const F_DIR: u8 = 1;
pub const F_INACCESSIBLE: u8 = 2;
pub const F_REPARSE: u8 = 4;
pub const F_CLOUD: u8 = 8;
pub const F_DELETED: u8 = 16;
/// 快速扫描时为了显示层级补出来的上级目录，只统计了一部分内容
pub const F_PARTIAL: u8 = 32;
pub const F_VIRTUAL: u8 = 64;
/// 风险等级是从上级继承来的
pub const F_INHERITED: u8 = 128;

/// 子树汇总标记
pub const S_INACCESSIBLE: u8 = 1;
pub const S_REPARSE: u8 = 2;
/// 子树里有被规则明确识别为“谨慎”的项目（软件数据、个人文件等），删除时要输入名称确认
pub const S_STRICT: u8 = 4;

pub const NO_PARENT: u32 = u32::MAX;

const ATTR_HIDDEN: u32 = 0x2;
const ATTR_SYSTEM: u32 = 0x4;

pub struct Node {
    pub name: Box<str>,
    pub parent: u32,
    pub children: Vec<u32>,
    pub size: u64,
    pub files: u64,
    pub mtime: u64,
    pub attrs: u32,
    pub flags: u8,
    pub sub: u8,
    pub level: u8,
    pub max_level: u8,
    pub rule: u16,
}

impl Node {
    pub fn is_dir(&self) -> bool {
        self.flags & F_DIR != 0
    }
}

pub struct RawFile {
    pub name: Box<str>,
    pub size: u64,
    pub mtime: u64,
    pub attrs: u32,
    pub flags: u8,
}

pub struct RawDir {
    pub name: Box<str>,
    pub mtime: u64,
    pub attrs: u32,
    pub flags: u8,
    pub files: Vec<RawFile>,
    pub dirs: Vec<RawDir>,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanMode {
    Quick,
    Full,
}

pub struct Tree {
    pub nodes: Vec<Node>,
    pub root: u32,
    pub mode: ScanMode,
    /// 扫描的是哪个磁盘，如 "D:\"
    pub drive: String,
    /// 快速扫描汇总：命中的可清理位置
    pub hits: Vec<u32>,
}

#[derive(Serialize)]
pub struct NodeView {
    pub id: u32,
    pub name: String,
    pub dir: bool,
    pub size: u64,
    pub files: u64,
    /// Unix 毫秒
    pub mtime: i64,
    pub level: u8,
    pub max_level: u8,
    /// 删除这一项时实际按哪个等级处理（回收站、只清内容的文件夹与 max_level 不同）
    pub del_level: u8,
    pub children: u32,
    pub flags: u8,
    pub sub: u8,
    pub rule: Option<String>,
}

pub fn filetime_to_unix_ms(ft: u64) -> i64 {
    if ft == 0 {
        0
    } else {
        (ft / 10_000) as i64 - 11_644_473_600_000
    }
}

impl Tree {
    pub fn new(mode: ScanMode, drive: &str) -> Tree {
        Tree { nodes: Vec::new(), root: NO_PARENT, mode, drive: drive.to_string(), hits: Vec::new() }
    }

    fn push(&mut self, parent: u32, name: Box<str>, flags: u8, attrs: u32, mtime: u64, size: u64) -> u32 {
        let id = self.nodes.len() as u32;
        self.nodes.push(Node {
            name,
            parent,
            children: Vec::new(),
            size,
            files: 0,
            mtime,
            attrs,
            flags,
            sub: 0,
            level: 2,
            max_level: 2,
            rule: H_DEFAULT,
        });
        if parent != NO_PARENT {
            self.nodes[parent as usize].children.push(id);
        } else if self.root == NO_PARENT {
            self.root = id;
        }
        id
    }

    pub fn push_virtual_root(&mut self, name: &str) -> u32 {
        self.push(NO_PARENT, name.into(), F_DIR | F_VIRTUAL, 0, 0, 0)
    }

    pub fn push_raw_dir(&mut self, parent: u32, raw: RawDir) -> u32 {
        let id = self.push(parent, raw.name, raw.flags | F_DIR, raw.attrs, raw.mtime, 0);
        for f in raw.files {
            self.push(id, f.name, f.flags, f.attrs, f.mtime, f.size);
        }
        for d in raw.dirs {
            self.push_raw_dir(id, d);
        }
        id
    }

    pub fn push_raw_file(&mut self, parent: u32, f: RawFile) -> u32 {
        self.push(parent, f.name, f.flags, f.attrs, f.mtime, f.size)
    }

    /// 快速扫描：按路径找到或补出上级目录（标记为“部分统计”）
    pub fn ensure_dir(&mut self, root: u32, comps: &[String]) -> u32 {
        let mut cur = root;
        for c in comps {
            let lower = c.to_lowercase();
            let found = self.nodes[cur as usize]
                .children
                .iter()
                .copied()
                .find(|&ch| self.nodes[ch as usize].name.to_lowercase() == lower);
            cur = match found {
                Some(ch) => ch,
                None => {
                    let path = {
                        let mut p = self.path(cur);
                        p.push(c);
                        p
                    };
                    let (attrs, mtime) = std::fs::symlink_metadata(&path)
                        .map(|m| {
                            use std::os::windows::fs::MetadataExt;
                            (m.file_attributes(), m.last_write_time())
                        })
                        .unwrap_or((0, 0));
                    self.push(cur, c.as_str().into(), F_DIR | F_PARTIAL, attrs, mtime, 0)
                }
            };
        }
        cur
    }

    pub fn path(&self, id: u32) -> PathBuf {
        let mut names: Vec<&str> = Vec::new();
        let mut cur = id;
        while cur != NO_PARENT {
            let n = &self.nodes[cur as usize];
            if n.flags & F_VIRTUAL == 0 {
                names.push(&n.name);
            }
            cur = n.parent;
        }
        let mut p = PathBuf::new();
        for n in names.iter().rev() {
            p.push(n);
        }
        p
    }

    pub fn is_ancestor(&self, anc: u32, mut id: u32) -> bool {
        while id != NO_PARENT {
            id = self.nodes[id as usize].parent;
            if id == anc {
                return true;
            }
        }
        false
    }

    /// 扫描完成后调用：判定风险 → 汇总大小 → 按大小排序
    pub fn finalize(&mut self, rules: &Rules) {
        self.classify(rules);
        self.aggregate();
        self.sort_children();
    }

    fn classify(&mut self, rules: &Rules) {
        let n = self.nodes.len();
        let mut depth = vec![0u16; n];
        // 每个节点“传给子项”的候选规则：(规则编号, 下一个要匹配的路径段)
        let mut cands: Vec<Vec<(u16, u8)>> = (0..n).map(|_| Vec::new()).collect();
        let root_list: Vec<(u16, u8)> = rules.root_cands.iter().map(|&p| (p, 0)).collect();
        // 每个节点传给子项的上下文：(条目, 等级, 是否受保护继承, 是否已在可清理命中范围内)
        let mut ctx: Vec<(u16, u8, bool, bool)> = vec![(H_DEFAULT, 2, false, false); n];
        self.hits.clear();

        for i in 0..n {
            let p = self.nodes[i].parent;
            let flags = self.nodes[i].flags;
            if flags & F_VIRTUAL != 0 {
                let node = &mut self.nodes[i];
                node.rule = H_VIRTUAL;
                node.level = 3;
                continue;
            }
            let parent_virtual = p == NO_PARENT || self.nodes[p as usize].flags & F_VIRTUAL != 0;
            let d = if parent_virtual { 0 } else { depth[p as usize] as usize + 1 };
            depth[i] = d as u16;
            let (pc_rule, pc_level, pc_prot, pc_hit) =
                if parent_virtual { (H_DEFAULT, 2, false, false) } else { ctx[p as usize] };
            let is_dir = flags & F_DIR != 0;
            let name_l = self.nodes[i].name.trim_end_matches('\\').to_lowercase();

            // 逐段匹配：继续上级传下来的候选；`**\` 规则可以在任意一层开始（受保护范围和软件目录里不开始）
            let parent_cands: &[(u16, u8)] = if parent_virtual { &root_list } else { &cands[p as usize] };
            let mut exact: Option<u16> = None;
            let mut next = Vec::new();
            let mut advance = |pid: u16, k: usize| {
                let pat = &rules.patterns[pid as usize];
                if pat.segs.len() <= k || !pat.segs[k].matches(&name_l) {
                    return;
                }
                if pat.segs.len() == k + 1 {
                    let e = rules.entry(pat.entry);
                    // 条目编号越小优先级越高（保护名单排在最前）
                    if !e.dir_only || is_dir {
                        exact = Some(exact.map_or(pat.entry, |x| x.min(pat.entry)));
                    }
                } else if is_dir {
                    next.push((pid, (k + 1) as u8));
                }
            };
            for &(pid, k) in parent_cands {
                advance(pid, k as usize);
            }
            if !pc_prot && rules.entry(pc_rule).allow_global {
                for &g in &rules.globals {
                    advance(g, 0);
                }
            }
            if is_dir && !next.is_empty() {
                cands[i] = next;
            }

            let attrs = self.nodes[i].attrs;
            let (rule, level, inherited, child_ctx);
            if d == 0 {
                rule = H_DRIVE_ROOT;
                level = 3;
                inherited = false;
                child_ctx = (H_DEFAULT, 2, false, false);
            } else if let Some(e) = exact {
                let ent = rules.entry(e);
                rule = e;
                level = ent.level;
                inherited = false;
                child_ctx = if ent.origin == Origin::Protected {
                    if ent.inherit { (e, 3, true, pc_hit) } else { (H_DEFAULT, 2, false, pc_hit) }
                } else {
                    let is_hit = ent.origin == Origin::Rule && ent.level <= 1;
                    if is_hit && !pc_hit {
                        self.hits.push(i as u32);
                    }
                    (e, ent.level, false, pc_hit || is_hit)
                };
            } else if is_dir && flags & F_REPARSE != 0 {
                let legacy = attrs & ATTR_HIDDEN != 0 && attrs & ATTR_SYSTEM != 0;
                rule = if legacy { H_LEGACY_JUNCTION } else { H_REPARSE };
                level = if legacy { 3 } else { pc_level.max(2) };
                inherited = false;
                child_ctx = (rule, level, false, pc_hit);
            } else if !is_dir && pc_rule == H_DEFAULT && is_temp_ext(&name_l) {
                rule = H_EXT_TEMP;
                level = 1;
                inherited = false;
                child_ctx = (pc_rule, pc_level, pc_prot, pc_hit);
            } else {
                rule = pc_rule;
                level = pc_level;
                inherited = true;
                child_ctx = (pc_rule, pc_level, pc_prot, pc_hit);
            }
            ctx[i] = child_ctx;
            let strict = level == 2 && rules.entry(rule).origin == Origin::Rule;
            let node = &mut self.nodes[i];
            node.rule = rule;
            node.level = level;
            if inherited {
                node.flags |= F_INHERITED;
            }
            if strict {
                node.sub |= S_STRICT;
            }
        }
    }

    fn aggregate(&mut self) {
        for i in 0..self.nodes.len() {
            let node = &mut self.nodes[i];
            node.max_level = node.level;
            node.sub &= S_STRICT; // 判定阶段设置的标记保留，其余重新汇总
            if node.is_dir() {
                node.size = 0;
                node.files = 0;
                if node.flags & F_INACCESSIBLE != 0 {
                    node.sub |= S_INACCESSIBLE;
                }
                if node.flags & F_REPARSE != 0 {
                    node.sub |= S_REPARSE;
                }
            } else {
                node.files = 1;
            }
        }
        for i in (0..self.nodes.len()).rev() {
            let (p, size, files, ml, sub) = {
                let n = &self.nodes[i];
                (n.parent, n.size, n.files, n.max_level, n.sub)
            };
            if p != NO_PARENT {
                let pn = &mut self.nodes[p as usize];
                pn.size += size;
                pn.files += files;
                pn.max_level = pn.max_level.max(ml);
                pn.sub |= sub;
            }
        }
    }

    fn sort_children(&mut self) {
        for i in 0..self.nodes.len() {
            if self.nodes[i].children.len() > 1 {
                let mut ch = std::mem::take(&mut self.nodes[i].children);
                ch.sort_unstable_by(|a, b| self.nodes[*b as usize].size.cmp(&self.nodes[*a as usize].size));
                self.nodes[i].children = ch;
            }
        }
    }

    pub fn view(&self, rules: &Rules, id: u32) -> NodeView {
        let n = &self.nodes[id as usize];
        let live_children = n.children.len() as u32;
        // 汇总节点的名称按当前语言生成
        let name = if n.flags & F_VIRTUAL != 0 {
            let d = self.drive.trim_end_matches('\\');
            match self.mode {
                ScanMode::Quick => format!("{} {d}", crate::i18n::tr("快速扫描", "Quick scan")),
                ScanMode::Full => crate::i18n::tr("扫描结果", "Scan results"),
            }
        } else {
            n.name.to_string()
        };
        NodeView {
            id,
            name,
            dir: n.is_dir(),
            size: n.size,
            files: n.files,
            mtime: filetime_to_unix_ms(n.mtime),
            level: n.level,
            max_level: n.max_level,
            del_level: self.delete_mode(rules, id).1,
            children: live_children,
            flags: n.flags,
            sub: n.sub,
            rule: if n.rule == H_DEFAULT { None } else { Some(rules.entry(n.rule).name.get().to_string()) },
        }
    }

    /// 删除这一项时的方式和风险等级
    pub fn delete_mode(&self, rules: &Rules, id: u32) -> (Mode, u8) {
        let n = &self.nodes[id as usize];
        let exact = n.flags & F_INHERITED == 0 && n.is_dir();
        let mode = if exact { rules.entry(n.rule).mode } else { Mode::Whole };
        let level = match mode {
            Mode::EmptyRecycleBin => n.level,
            Mode::Contents => n.children.iter().map(|&c| self.nodes[c as usize].max_level).max().unwrap_or(0).max(n.level),
            Mode::Whole => n.max_level,
        };
        (mode, level)
    }

    /// 第一个受保护（L3）的子孙节点
    pub fn first_protected_descendant(&self, id: u32) -> Option<u32> {
        let mut stack = vec![id];
        while let Some(cur) = stack.pop() {
            for &ch in &self.nodes[cur as usize].children {
                let c = &self.nodes[ch as usize];
                if c.level >= 3 {
                    return Some(ch);
                }
                if c.max_level >= 3 {
                    stack.push(ch);
                }
            }
        }
        None
    }

    /// 从树中移除一个已删除的节点，并更新上级的大小和风险
    pub fn remove(&mut self, id: u32) {
        let (size, files, parent) = {
            let n = &mut self.nodes[id as usize];
            if n.flags & F_DELETED != 0 {
                return;
            }
            n.flags |= F_DELETED;
            (n.size, n.files, n.parent)
        };
        if parent == NO_PARENT {
            return;
        }
        self.nodes[parent as usize].children.retain(|&c| c != id);
        let mut cur = parent;
        while cur != NO_PARENT {
            let max_child = self.nodes[cur as usize]
                .children
                .iter()
                .map(|&c| self.nodes[c as usize].max_level)
                .max()
                .unwrap_or(0);
            let n = &mut self.nodes[cur as usize];
            n.size = n.size.saturating_sub(size);
            n.files = n.files.saturating_sub(files);
            n.max_level = n.level.max(max_child);
            cur = n.parent;
        }
    }
}

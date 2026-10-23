//! 删除：先生成删除计划（按风险分级、拦截受保护项），执行时后端再次校验确认信息。

use crate::i18n::tr;
use crate::rules::{Mode, Rules};
use crate::tree::*;
use crate::winutil;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Write;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

const ATTR_READONLY: u32 = 0x1;
const ATTR_DIRECTORY: u32 = 0x10;
const ATTR_REPARSE: u32 = 0x400;

/// 多选删除时要输入的确认文字
pub fn multi_confirm_text() -> String {
    tr("确认删除", "DELETE")
}

#[derive(Serialize, Clone)]
pub struct PlanItem {
    pub id: u32,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub level: u8,
    pub rule_name: String,
    pub note: String,
    pub alt: Option<String>,
    pub mode: Mode,
    pub blocked: Option<String>,
    /// “谨慎”来自规则的明确识别（而不只是未识别），需要输入名称确认
    pub strict: bool,
}

#[derive(Serialize)]
pub struct Plan {
    pub items: Vec<PlanItem>,
    pub blocked: Vec<PlanItem>,
    pub total: u64,
    pub max_level: u8,
    /// 需要手动输入的文字（仅 typing_required 时使用）
    pub expected_text: String,
    /// 有明确识别为“谨慎”的项目时才需要输入；全是未识别的项目只需勾选“我已了解”
    pub typing_required: bool,
    pub permanent_allowed: bool,
}

#[derive(Deserialize)]
pub struct DeleteRequest {
    pub ids: Vec<u32>,
    pub permanent: bool,
    pub ack: bool,
    pub typed: String,
}

#[derive(Serialize, Default)]
pub struct Failure {
    pub path: String,
    pub reason: String,
}

#[derive(Serialize, Default)]
pub struct Outcome {
    pub freed: u64,
    pub removed: u64,
    pub failed: Vec<Failure>,
}

pub fn build_plan(tree: &Tree, rules: &Rules, ids: &[u32]) -> Plan {
    // 去掉无效的、已删除的，以及上级已被选中的
    let set: HashSet<u32> = ids
        .iter()
        .copied()
        .filter(|&i| (i as usize) < tree.nodes.len() && tree.nodes[i as usize].flags & F_DELETED == 0)
        .collect();
    let mut kept: Vec<u32> = set.iter().copied().filter(|&i| !set.iter().any(|&a| a != i && tree.is_ancestor(a, i))).collect();
    kept.sort_by(|a, b| tree.nodes[*b as usize].size.cmp(&tree.nodes[*a as usize].size));

    let mut items = Vec::new();
    let mut blocked = Vec::new();
    for id in kept {
        let n = &tree.nodes[id as usize];
        let e = rules.entry(n.rule);
        let (mode, mut level) = tree.delete_mode(rules, id);
        let strict = level >= 2
            && match mode {
                Mode::Whole => n.sub & S_STRICT != 0,
                Mode::Contents => n.children.iter().any(|&c| tree.nodes[c as usize].sub & S_STRICT != 0),
                Mode::EmptyRecycleBin => false,
            };
        let mut note = e.note.get().to_string();
        if mode != Mode::EmptyRecycleBin && n.sub & S_INACCESSIBLE != 0 && level < 2 {
            level = 2;
            note.push_str(&tr("（注意：里面有没有权限读取的子文件夹，内容未知，因此按“谨慎”处理。）", " (Note: some subfolders could not be read, so their contents are unknown. Treated as “Caution”.)"));
        }
        let mut item = PlanItem {
            id,
            name: n.name.to_string(),
            path: tree.path(id).display().to_string(),
            size: n.size,
            level,
            rule_name: e.name.get().to_string(),
            note,
            alt: e.alt.as_ref().map(|a| a.get().to_string()),
            mode,
            blocked: None,
            strict,
        };
        if n.flags & F_VIRTUAL != 0 {
            item.blocked = Some(tr("这是扫描结果的汇总节点，不对应真实的文件夹。", "This is a summary node of the scan results, not a real folder."));
        } else if n.flags & F_PARTIAL != 0 {
            item.blocked = Some(tr("快速扫描只统计了这个文件夹里的一部分内容，无法判断其余内容是否安全。请用全盘扫描后再操作，或展开后选择具体的项目。", "Quick scan only counted part of this folder, so the rest cannot be judged safe. Run a full scan first, or expand it and pick specific items."));
        } else if level >= 3 {
            if n.level >= 3 {
                item.blocked = Some(e.note.get().to_string());
            } else {
                let p = tree.first_protected_descendant(id).map(|d| tree.path(d).display().to_string()).unwrap_or_default();
                item.blocked = Some(if crate::i18n::is_en() { format!("Contains protected items: {p}. Expand it and pick only the parts that can be deleted.") } else { format!("里面包含受保护的项目：{p}。请展开后只选择可以删除的部分。") });
                item.alt = None;
            }
        }
        if item.blocked.is_some() {
            blocked.push(item);
        } else {
            items.push(item);
        }
    }
    let total = items.iter().map(|i| i.size).sum();
    let max_level = items.iter().map(|i| i.level).max().unwrap_or(0);
    let expected_text = if items.len() == 1 { items[0].name.clone() } else { multi_confirm_text() };
    let permanent_allowed = !items.is_empty() && max_level == 0;
    let typing_required = items.iter().any(|i| i.strict);
    Plan { items, blocked, total, max_level, expected_text, typing_required, permanent_allowed }
}

/// 后端校验：前端界面被绕过时也不能删除危险项
pub fn check_request(plan: &Plan, req: &DeleteRequest) -> Result<(), String> {
    if plan.items.is_empty() {
        return Err(tr("没有可以删除的项目", "Nothing that can be deleted"));
    }
    if req.permanent && !plan.permanent_allowed {
        return Err(tr("只有全部是“安全”级别的项目时才能永久删除", "Permanent delete is only allowed when every item is “Safe”"));
    }
    if plan.max_level >= 2 {
        if !req.ack {
            return Err(tr("需要先勾选“我已了解”，已取消删除", "“I understand” was not ticked. Delete cancelled"));
        }
        if plan.typing_required && req.typed.trim() != plan.expected_text {
            return Err(tr("输入的确认文字不正确，已取消删除", "The confirmation text does not match. Delete cancelled"));
        }
    }
    Ok(())
}

/// 删除前要执行的具体动作（从树中取出，执行时不再持有树的锁）
pub struct Job {
    pub item: PlanItem,
    /// 执行后要检查是否还存在的节点（Contents / 回收站模式下是子节点）
    pub check: Vec<(u32, PathBuf)>,
}

pub fn jobs_from_plan(tree: &Tree, plan: &Plan) -> Vec<Job> {
    plan.items
        .iter()
        .map(|it| {
            let n = &tree.nodes[it.id as usize];
            let check = match it.mode {
                Mode::Whole => vec![(it.id, PathBuf::from(&it.path))],
                Mode::Contents => n.children.iter().map(|&c| (c, tree.path(c))).collect(),
                // 回收站下面是每个用户的文件夹，清空后它们还在，检查的是里面的内容
                Mode::EmptyRecycleBin => n
                    .children
                    .iter()
                    .flat_map(|&c| tree.nodes[c as usize].children.iter().map(|&g| (g, tree.path(g))))
                    .collect(),
            };
            Job { item: it.clone(), check }
        })
        .collect()
}

fn exists(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}

/// 路径的上级里有没有链接（防止删到别的位置去）
fn has_reparse_ancestor(p: &Path) -> bool {
    p.ancestors().skip(1).filter(|a| a.parent().is_some()).any(|a| {
        std::fs::symlink_metadata(a).map(|m| m.file_attributes() & ATTR_REPARSE != 0).unwrap_or(false)
    })
}

/// 永久删除，碰到正在使用的文件就跳过继续
fn remove_best_effort(p: &Path, errs: &mut Vec<String>) {
    let md = match std::fs::symlink_metadata(p) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => return errs.push(format!("{}: {e}", p.display())),
    };
    let a = md.file_attributes();
    if a & ATTR_DIRECTORY != 0 {
        // 链接只删链接本身，不进入
        if a & ATTR_REPARSE == 0 {
            if let Ok(rd) = std::fs::read_dir(p) {
                for ent in rd.flatten() {
                    remove_best_effort(&ent.path(), errs);
                }
            }
        }
        if let Err(e) = std::fs::remove_dir(p) {
            errs.push(format!("{}: {e}", p.display()));
        }
    } else {
        if a & ATTR_READONLY != 0 {
            let mut perm = md.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perm.set_readonly(false);
            let _ = std::fs::set_permissions(p, perm);
        }
        if let Err(e) = std::fs::remove_file(p) {
            errs.push(format!("{}: {e}", p.display()));
        }
    }
}

/// 真正执行删除（不碰树）。返回每个任务的错误信息。
pub fn run_jobs(jobs: &[Job], permanent: bool, owner: Option<isize>) -> Vec<Option<String>> {
    jobs.iter()
        .map(|job| {
            let path = PathBuf::from(&job.item.path);
            if has_reparse_ancestor(&path) {
                return Some(tr("路径中包含链接，为安全起见已跳过", "The path goes through a link, skipped for safety"));
            }
            match job.item.mode {
                Mode::EmptyRecycleBin => {
                    let drive: String = job.item.path.chars().take(3).collect();
                    winutil::empty_recycle_bin(&drive).err()
                }
                Mode::Contents | Mode::Whole => {
                    let targets: Vec<PathBuf> = if job.item.mode == Mode::Whole {
                        vec![path]
                    } else {
                        // 按删除时的实际内容来删
                        std::fs::read_dir(&path).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default()
                    };
                    if permanent {
                        let mut errs = Vec::new();
                        for t in &targets {
                            remove_best_effort(t, &mut errs);
                        }
                        (!errs.is_empty()).then(|| if crate::i18n::is_en() { format!("{} files or folders could not be deleted (in use or no permission), e.g. {}", errs.len(), errs[0]) } else { format!("有 {} 个文件或文件夹未能删除（可能正在使用或没有权限），例如：{}", errs.len(), errs[0]) })
                    } else {
                        match winutil::recycle(&targets, owner) {
                            Ok(false) => None,
                            Ok(true) => Some(tr("部分项目未能移到回收站（可能正在使用、没有权限，或在提示中选择了取消）", "Some items could not be moved to the Recycle Bin (in use, no permission, or cancelled in the prompt)")),
                            Err(e) => Some(e),
                        }
                    }
                }
            }
        })
        .collect()
}

/// 执行后按实际情况更新树
pub fn apply_results(tree: &mut Tree, jobs: &[Job], errs: Vec<Option<String>>) -> Outcome {
    let mut out = Outcome::default();
    for (job, err) in jobs.iter().zip(errs) {
        let mut any_left = false;
        for (id, p) in &job.check {
            if exists(p) {
                any_left = true;
            } else if tree.nodes[*id as usize].flags & F_DELETED == 0 {
                out.freed += tree.nodes[*id as usize].size;
                out.removed += 1;
                tree.remove(*id);
            }
        }
        if let Some(reason) = err {
            out.failed.push(Failure { path: job.item.path.clone(), reason });
        } else if any_left && job.item.mode == Mode::Whole {
            out.failed.push(Failure { path: job.item.path.clone(), reason: tr("未能完全删除，部分文件可能正在使用", "Not fully deleted; some files may be in use") });
        }
    }
    out
}

pub fn log_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    base.join("DiskCleaner")
}

pub fn write_log(plan: &Plan, req: &DeleteRequest, out: &Outcome) {
    let dir = log_dir();
    let _ = std::fs::create_dir_all(&dir);
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let line = serde_json::json!({
        "time": ts,
        "method": if req.permanent { "永久删除" } else { "回收站" },
        "max_level": plan.max_level,
        "confirmed_with_text": plan.typing_required,
        "items": plan.items.iter().map(|i| serde_json::json!({"path": i.path, "size": i.size, "level": i.level, "rule": i.rule_name, "mode": i.mode})).collect::<Vec<_>>(),
        "freed": out.freed,
        "failed": out.failed.iter().map(|f| serde_json::json!({"path": f.path, "reason": f.reason})).collect::<Vec<_>>(),
    });
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("delete-log.jsonl")) {
        let _ = writeln!(f, "{line}");
    }
}

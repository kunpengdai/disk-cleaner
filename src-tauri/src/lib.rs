mod delete;
mod rules;
mod scanner;
mod tree;
mod winutil;

use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager, State};
use tree::*;

struct AppState {
    rules: Arc<rules::Rules>,
    tree: Arc<RwLock<Option<Tree>>>,
    /// 正在扫描或删除
    busy: Arc<AtomicBool>,
    cancel: Mutex<Option<Arc<AtomicBool>>>,
}

#[derive(Serialize, Clone)]
struct ProgressEvent {
    files: u64,
    dirs: u64,
    bytes: u64,
    errors: u64,
    current: String,
    elapsed_ms: u64,
}

#[derive(Serialize, Clone)]
struct FinishedEvent {
    ok: bool,
    cancelled: bool,
    elapsed_ms: u64,
    nodes: usize,
    error: Option<String>,
}

#[derive(Serialize)]
struct ChildrenPage {
    items: Vec<NodeView>,
    total: usize,
}

#[derive(Serialize)]
struct NodeDetail {
    node: NodeView,
    path: String,
    rule_id: String,
    rule_name: String,
    category: String,
    origin: rules::Origin,
    inherited: bool,
    note: String,
    alt: Option<String>,
    mode: rules::Mode,
    protected_inside: Option<String>,
}

#[derive(Serialize)]
struct Hit {
    node: NodeView,
    path: String,
    category: String,
    note: String,
}

#[derive(Serialize)]
struct TreeInfo {
    root: NodeView,
    mode: ScanMode,
    drive: String,
}

type R<T> = Result<T, String>;

#[tauri::command]
async fn list_drives() -> R<Vec<winutil::DriveInfo>> {
    Ok(winutil::list_drives())
}

#[tauri::command]
async fn is_elevated() -> R<bool> {
    Ok(winutil::is_elevated())
}

#[tauri::command]
async fn relaunch_as_admin(app: tauri::AppHandle) -> R<()> {
    winutil::relaunch_as_admin()?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
async fn start_scan(app: tauri::AppHandle, state: State<'_, AppState>, mode: String, root: Option<String>) -> R<()> {
    if state.busy.swap(true, Relaxed) {
        return Err("正在扫描或删除，请稍候".into());
    }
    // 扫描期间不保留旧结果，避免拿过期的数据去删除
    *state.tree.write() = None;
    let cancel = Arc::new(AtomicBool::new(false));
    *state.cancel.lock() = Some(cancel.clone());
    let (rules, tree_slot, busy) = (state.rules.clone(), state.tree.clone(), state.busy.clone());

    std::thread::spawn(move || {
        let progress = Arc::new(scanner::Progress::default());
        let start = Instant::now();
        let done = Arc::new(AtomicBool::new(false));
        // 定时推送进度
        let ticker = {
            let (app, progress, done) = (app.clone(), progress.clone(), done.clone());
            std::thread::spawn(move || {
                while !done.load(Relaxed) {
                    let _ = app.emit("scan-progress", ProgressEvent {
                        files: progress.files.load(Relaxed),
                        dirs: progress.dirs.load(Relaxed),
                        bytes: progress.bytes.load(Relaxed),
                        errors: progress.errors.load(Relaxed),
                        current: progress.current.lock().clone(),
                        elapsed_ms: start.elapsed().as_millis() as u64,
                    });
                    std::thread::sleep(Duration::from_millis(150));
                }
            })
        };
        let ctx = scanner::Ctx { progress: &progress, cancel: &cancel };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let root = PathBuf::from(root.unwrap_or_else(|| "C:\\".into()));
            if !root.is_dir() {
                return Err(format!("找不到 {}", root.display()));
            }
            if mode == "quick" {
                Ok(scanner::scan_quick(&rules, &root.to_string_lossy(), &ctx))
            } else {
                Ok(scanner::scan_full(&root, &rules, &ctx))
            }
        }));
        done.store(true, Relaxed);
        let _ = ticker.join();
        let cancelled = cancel.load(Relaxed);
        let mut ev = FinishedEvent { ok: false, cancelled, elapsed_ms: start.elapsed().as_millis() as u64, nodes: 0, error: None };
        match result {
            // 取消后的结果不完整，直接丢弃（否则没扫到的文件夹会被误判成空的）
            Ok(Ok(t)) if !cancelled => {
                ev.ok = true;
                ev.nodes = t.nodes.len();
                *tree_slot.write() = Some(t);
            }
            Ok(Ok(_)) => {}
            Ok(Err(e)) => ev.error = Some(e),
            Err(_) => ev.error = Some("扫描时发生内部错误".into()),
        }
        busy.store(false, Relaxed);
        let _ = app.emit("scan-finished", ev);
    });
    Ok(())
}

#[tauri::command]
async fn cancel_scan(state: State<'_, AppState>) -> R<()> {
    if let Some(c) = state.cancel.lock().as_ref() {
        c.store(true, Relaxed);
    }
    Ok(())
}

#[tauri::command]
async fn get_tree_info(state: State<'_, AppState>) -> R<Option<TreeInfo>> {
    let g = state.tree.read();
    Ok(g.as_ref().map(|t| TreeInfo { root: t.view(&state.rules, t.root), mode: t.mode, drive: t.drive.clone() }))
}

#[tauri::command]
async fn get_children(state: State<'_, AppState>, id: u32, sort: String, offset: usize, limit: usize) -> R<ChildrenPage> {
    let g = state.tree.read();
    let t = g.as_ref().ok_or("还没有扫描结果")?;
    let n = t.nodes.get(id as usize).ok_or("节点不存在")?;
    let mut ids = n.children.clone();
    match sort.as_str() {
        "name" => ids.sort_by_cached_key(|&c| t.nodes[c as usize].name.to_lowercase()),
        "files" => ids.sort_by(|a, b| t.nodes[*b as usize].files.cmp(&t.nodes[*a as usize].files)),
        "mtime" => ids.sort_by(|a, b| t.nodes[*b as usize].mtime.cmp(&t.nodes[*a as usize].mtime)),
        _ => {} // 默认已按大小排好
    }
    let total = ids.len();
    let items = ids.into_iter().skip(offset).take(limit).map(|c| t.view(&state.rules, c)).collect();
    Ok(ChildrenPage { items, total })
}

#[tauri::command]
async fn get_detail(state: State<'_, AppState>, id: u32) -> R<NodeDetail> {
    let g = state.tree.read();
    let t = g.as_ref().ok_or("还没有扫描结果")?;
    let n = t.nodes.get(id as usize).ok_or("节点不存在")?;
    let e = state.rules.entry(n.rule);
    let inherited = n.flags & F_INHERITED != 0;
    let protected_inside = if n.level < 3 && n.max_level >= 3 {
        t.first_protected_descendant(id).map(|d| t.path(d).display().to_string())
    } else {
        None
    };
    Ok(NodeDetail {
        node: t.view(&state.rules, id),
        path: t.path(id).display().to_string(),
        rule_id: e.id.clone(),
        rule_name: e.name.clone(),
        category: e.category.clone(),
        origin: e.origin,
        inherited,
        note: e.note.clone(),
        alt: e.alt.clone(),
        mode: if inherited || !n.is_dir() { rules::Mode::Whole } else { e.mode },
        protected_inside,
    })
}

#[tauri::command]
async fn get_hits(state: State<'_, AppState>) -> R<Vec<Hit>> {
    let g = state.tree.read();
    let Some(t) = g.as_ref() else { return Ok(Vec::new()) };
    let mut hits: Vec<Hit> = t
        .hits
        .iter()
        .filter(|&&h| t.nodes[h as usize].flags & F_DELETED == 0 && t.nodes[h as usize].size > 0)
        .map(|&h| {
            let e = state.rules.entry(t.nodes[h as usize].rule);
            Hit { node: t.view(&state.rules, h), path: t.path(h).display().to_string(), category: e.category.clone(), note: e.note.clone() }
        })
        .collect();
    hits.sort_by(|a, b| b.node.size.cmp(&a.node.size));
    Ok(hits)
}

#[tauri::command]
async fn plan_delete(state: State<'_, AppState>, ids: Vec<u32>) -> R<delete::Plan> {
    let g = state.tree.read();
    let t = g.as_ref().ok_or("还没有扫描结果")?;
    Ok(delete::build_plan(t, &state.rules, &ids))
}

#[tauri::command]
async fn execute_delete(window: tauri::WebviewWindow, state: State<'_, AppState>, req: delete::DeleteRequest) -> R<delete::Outcome> {
    if state.busy.swap(true, Relaxed) {
        return Err("正在扫描或删除，请稍候".into());
    }
    let result = (|| {
        let (plan, jobs) = {
            let g = state.tree.read();
            let t = g.as_ref().ok_or("还没有扫描结果")?;
            let plan = delete::build_plan(t, &state.rules, &req.ids);
            delete::check_request(&plan, &req)?;
            let jobs = delete::jobs_from_plan(t, &plan);
            (plan, jobs)
        };
        Ok::<_, String>((plan, jobs))
    })();
    let (plan, jobs) = match result {
        Ok(v) => v,
        Err(e) => {
            state.busy.store(false, Relaxed);
            return Err(e);
        }
    };
    let owner = window.hwnd().ok().map(|h| h.0 as isize);
    let permanent = req.permanent;
    let (jobs, errs) = tauri::async_runtime::spawn_blocking(move || {
        let errs = delete::run_jobs(&jobs, permanent, owner);
        (jobs, errs)
    })
    .await
    .map_err(|e| {
        state.busy.store(false, Relaxed);
        e.to_string()
    })?;
    let out = {
        let mut g = state.tree.write();
        match g.as_mut() {
            Some(t) => delete::apply_results(t, &jobs, errs),
            None => delete::Outcome::default(),
        }
    };
    delete::write_log(&plan, &req, &out);
    state.busy.store(false, Relaxed);
    Ok(out)
}

#[tauri::command]
async fn open_in_explorer(state: State<'_, AppState>, id: u32) -> R<()> {
    let g = state.tree.read();
    let t = g.as_ref().ok_or("还没有扫描结果")?;
    winutil::open_in_explorer(&t.path(id));
    Ok(())
}

#[tauri::command]
async fn open_log_dir() -> R<()> {
    let d = delete::log_dir();
    let _ = std::fs::create_dir_all(&d);
    let _ = std::process::Command::new("explorer").arg(d).spawn();
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(AppState {
                rules: Arc::new(rules::Rules::load()),
                tree: Arc::new(RwLock::new(None)),
                busy: Arc::new(AtomicBool::new(false)),
                cancel: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_drives,
            is_elevated,
            relaunch_as_admin,
            start_scan,
            cancel_scan,
            get_tree_info,
            get_children,
            get_detail,
            get_hits,
            plan_delete,
            execute_delete,
            open_in_explorer,
            open_log_dir
        ])
        .run(tauri::generate_context!())
        .expect("启动失败");
}

#[cfg(test)]
mod tests;

import { listen } from "@tauri-apps/api/event";
import { api, badge, esc, F, fmtCount, fmtDate, fmtSize, type Drive, type NodeView } from "./api";
import { openDeleteFlow } from "./dialog";

const $ = (id: string) => document.getElementById(id)!;
const ROW_H = 28;
const PAGE = 500;

type Row =
  | { kind: "node"; node: NodeView; depth: number; parentSize: number }
  | { kind: "more"; parent: number; depth: number; loaded: number; total: number };

const state = {
  drives: [] as Drive[],
  drive: "C:\\",
  scanning: false,
  mode: null as "quick" | "full" | null,
  root: null as NodeView | null,
  /** 当前扫描结果属于哪个磁盘 */
  treeDrive: "",
  sort: "size",
  expanded: new Set<number>(),
  kids: new Map<number, { items: NodeView[]; total: number }>(),
  rows: [] as Row[],
  focus: -1,
  basket: new Map<number, NodeView>(),
};

// ---------- 磁盘与权限 ----------
async function loadDrives() {
  state.drives = await api.drives();
  if (!state.drives.some((d) => d.root === state.drive) && state.drives.length) state.drive = state.drives[0].root;
  $("drives").innerHTML = state.drives.map((d) => {
    const used = d.total - d.free;
    const pct = d.total ? (used / d.total) * 100 : 0;
    const scanned = state.root && state.treeDrive === d.root
      ? ` <span class="scanned">${state.mode === "quick" ? "已快速扫描" : "已全盘扫描"}</span>` : "";
    return `<button class="drive${d.root === state.drive ? " active" : ""}" data-root="${esc(d.root)}">
      <div class="dname">${esc(d.label || d.kind)} (${esc(d.root.slice(0, 2))})${scanned}</div>
      <div class="dmeta">可用 ${fmtSize(d.free)}，共 ${fmtSize(d.total)}</div>
      <div class="dbar"><div style="width:${pct.toFixed(1)}%;${pct > 90 ? "background:var(--danger)" : ""}"></div></div>
    </button>`;
  }).join("");
  $("drives").querySelectorAll<HTMLElement>(".drive").forEach((el) =>
    el.addEventListener("click", () => {
      if (state.drive === el.dataset.root) return;
      state.drive = el.dataset.root!;
      loadDrives();
      updateView();
    }));
  updateScanButtons();
}

function driveLabel(root: string): string {
  const d = state.drives.find((x) => x.root === root);
  return d ? `${d.label || d.kind} (${root.slice(0, 2)})` : root.slice(0, 2);
}

function updateScanButtons() {
  const dl = state.drive.slice(0, 2);
  $("btn-quick").textContent = `快速扫描 ${dl}`;
  $("btn-full").textContent = `全盘扫描 ${dl}`;
}

/** 当前选中的磁盘是否就是扫描结果所在的磁盘 */
function visible(): boolean {
  return !!state.root && state.treeDrive === state.drive;
}

/** 切换磁盘或扫描结果变化后，刷新下方区域 */
function updateView() {
  const show = visible();
  $("empty").classList.toggle("hidden", show);
  if (!show) renderEmpty();
  renderRows();
  if (show) {
    loadHits();
    if (state.focus >= 0) { const r = state.rows[state.focus]; if (r?.kind === "node") showDetail(r.node.id); }
  } else {
    $("tab-detail").innerHTML = `<p class="muted">点击左侧的一行查看详情。</p>`;
    $("tab-hits").innerHTML = `<p class="muted">扫描 ${esc(state.drive.slice(0, 2))} 后，这里会列出可以清理的项目。</p>`;
  }
  renderBasket();
}

function renderEmpty() {
  const d = state.drives.find((x) => x.root === state.drive);
  const used = d ? d.total - d.free : 0;
  const other = state.root && state.treeDrive !== state.drive
    ? `<p class="muted" style="margin-top:14px">目前保存的是 ${esc(driveLabel(state.treeDrive))} 的扫描结果，点击上方的该磁盘可以返回查看。扫描 ${esc(state.drive.slice(0, 2))} 后会替换它。</p>` : "";
  $("empty").innerHTML = `
    <h2>${esc(driveLabel(state.drive))}${state.scanning ? " · 正在扫描…" : " 还没有扫描"}</h2>
    ${d ? `<p>已用 ${fmtSize(used)}，可用 ${fmtSize(d.free)}，共 ${fmtSize(d.total)}${d.fs ? ` · ${esc(d.fs)}` : ""}</p>` : ""}
    <div class="empty-actions">
      <button class="primary" data-scan="quick" ${state.scanning ? "disabled" : ""}>快速扫描</button>
      <button data-scan="full" ${state.scanning ? "disabled" : ""}>全盘扫描</button>
    </div>
    <p><b>快速扫描</b>：只检查已知的缓存、临时文件、回收站等位置，通常几秒钟。</p>
    <p><b>全盘扫描</b>：统计这个磁盘里每个文件夹的大小，可能需要一两分钟。</p>
    ${other}
    <div class="legend">
      <span class="badge l0">安全</span> 可直接清理
      <span class="badge l1">需确认</span> 确认一次
      <span class="badge l2">谨慎</span> 多重确认，只进回收站
      <span class="badge l3">受保护</span> 不能删除
    </div>`;
  $("empty").querySelectorAll<HTMLElement>("[data-scan]").forEach((b) =>
    b.addEventListener("click", () => startScan(b.dataset.scan as "quick" | "full")));
}

async function loadAdmin() {
  const elevated = await api.isElevated();
  $("admin").innerHTML = elevated
    ? `<span class="badge l0">管理员</span>`
    : `<span>普通权限</span><button class="link" id="btn-elevate" title="可以扫描和清理系统目录，例如 Windows 更新缓存">以管理员身份重启</button>`;
  document.getElementById("btn-elevate")?.addEventListener("click", async () => {
    try { await api.relaunchAsAdmin(); } catch (e) { toast(String(e)); }
  });
}

// ---------- 扫描 ----------
async function startScan(mode: "quick" | "full") {
  if (state.scanning) return;
  try {
    await api.startScan(mode, state.drive);
  } catch (e) { toast(String(e)); return; }
  state.scanning = true;
  state.mode = mode;
  state.basket.clear();
  resetTree();
  setScanningUi(true);
  const dl = state.drive.slice(0, 2);
  $("progress-text").textContent = mode === "quick" ? `正在检查 ${dl} 中已知的可清理位置…` : `正在扫描 ${dl} …`;
}

function setScanningUi(on: boolean) {
  $("progress").classList.toggle("hidden", !on);
  $("btn-cancel").classList.toggle("hidden", !on);
  ($("btn-quick") as HTMLButtonElement).disabled = on;
  ($("btn-full") as HTMLButtonElement).disabled = on;
}

listen<{ files: number; dirs: number; bytes: number; errors: number; current: string; elapsed_ms: number }>("scan-progress", (e) => {
  const p = e.payload;
  $("progress-text").textContent =
    `${(p.elapsed_ms / 1000).toFixed(1)} 秒 · ${fmtCount(p.files)} 个文件 · ${fmtCount(p.dirs)} 个文件夹 · ${fmtSize(p.bytes)}`
    + (p.errors ? ` · ${p.errors} 处无权限` : "") + (p.current ? ` · ${p.current}` : "");
});

listen<{ ok: boolean; cancelled: boolean; elapsed_ms: number; nodes: number; error: string | null }>("scan-finished", async (e) => {
  state.scanning = false;
  setScanningUi(false);
  const r = e.payload;
  if (r.ok) {
    toast(`扫描完成，用时 ${(r.elapsed_ms / 1000).toFixed(1)} 秒`);
    await loadTree();
    if (state.mode === "quick") switchTab("hits");
  } else {
    toast(r.cancelled ? "已取消扫描" : r.error ?? "扫描失败");
    updateView();
  }
  loadDrives();
});

// ---------- 树 ----------
function resetTree() {
  state.root = null;
  state.treeDrive = "";
  state.expanded.clear();
  state.kids.clear();
  state.rows = [];
  state.focus = -1;
  updateView();
  if (state.drives.length) loadDrives();
}

async function loadTree() {
  const info = await api.treeInfo();
  if (!info) { resetTree(); return; }
  state.root = info.root;
  state.mode = info.mode;
  state.treeDrive = info.drive;
  state.drive = info.drive;
  state.expanded.add(info.root.id);
  // 快速扫描：自动展开到命中的位置所在层级
  await ensureKids(info.root.id);
  if (info.mode === "quick") {
    for (const d of state.kids.get(info.root.id)!.items) { state.expanded.add(d.id); await ensureKids(d.id); }
  }
  await rebuild();
  await loadDrives();
  updateView();
}

async function ensureKids(id: number, more = false) {
  const cur = state.kids.get(id);
  if (cur && !more) return;
  const offset = more && cur ? cur.items.length : 0;
  const page = await api.children(id, state.sort, offset, PAGE);
  if (more && cur) cur.items.push(...page.items), (cur.total = page.total);
  else state.kids.set(id, page);
}

async function rebuild() {
  const rows: Row[] = [];
  if (!state.root) { state.rows = rows; renderRows(); return; }
  const walk = (n: NodeView, depth: number, parentSize: number) => {
    rows.push({ kind: "node", node: n, depth, parentSize });
    if (!state.expanded.has(n.id)) return;
    const k = state.kids.get(n.id);
    if (!k) return;
    for (const c of k.items) walk(c, depth + 1, n.size);
    if (k.items.length < k.total) rows.push({ kind: "more", parent: n.id, depth: depth + 1, loaded: k.items.length, total: k.total });
  };
  walk(state.root, 0, state.root.size);
  state.rows = rows;
  renderRows();
}

function tags(n: NodeView): string {
  const t: string[] = [];
  if (n.flags & F.INACCESSIBLE) t.push("无权限");
  if (n.flags & F.REPARSE) t.push("链接");
  if (n.flags & F.CLOUD) t.push("仅云端");
  if (n.flags & F.PARTIAL) t.push("部分统计");
  return t.map((x) => `<span class="tag">[${x}]</span>`).join("");
}

function renderRows() {
  const tree = $("tree");
  const inner = $("tree-inner");
  const rows = visible() ? state.rows : [];
  inner.style.height = `${rows.length * ROW_H}px`;
  const top = tree.scrollTop;
  const first = Math.max(0, Math.floor(top / ROW_H) - 10);
  const last = Math.min(rows.length, Math.ceil((top + tree.clientHeight) / ROW_H) + 10);
  let html = "";
  for (let i = first; i < last; i++) {
    const r = rows[i];
    const y = i * ROW_H;
    const pad = r.depth * 18 + 6;
    if (r.kind === "more") {
      html += `<div class="row more" style="top:${y}px" data-i="${i}"><div class="c-name" style="padding-left:${pad + 20}px">显示更多（还有 ${r.total - r.loaded} 项）</div></div>`;
      continue;
    }
    const n = r.node;
    const canOpen = n.dir && n.children > 0;
    const open = state.expanded.has(n.id);
    const pct = r.parentSize ? (n.size / r.parentSize) * 100 : 0;
    const selectable = !(n.flags & F.VIRTUAL);
    html += `<div class="row${i === state.focus ? " focus" : ""}" style="top:${y}px" data-i="${i}">
      <div class="c-name" style="padding-left:${pad}px" title="${esc(n.name)}">
        <span class="toggle" data-toggle="${i}">${canOpen ? (open ? "▼" : "▶") : ""}</span>
        ${selectable ? `<input type="checkbox" data-pick="${i}" ${state.basket.has(n.id) ? "checked" : ""} ${n.del_level >= 3 ? 'title="受保护，不能删除"' : ""}/>` : ""}
        <span class="nm">${n.dir ? "📁" : "📄"} ${esc(n.name)}</span>${tags(n)}
      </div>
      <div class="c-size">${fmtSize(n.size)}</div>
      <div class="c-bar pbar"><div class="track"><div class="fill" style="width:${Math.min(100, pct).toFixed(1)}%"></div></div><div class="pct">${pct >= 0.1 ? pct.toFixed(pct >= 10 ? 0 : 1) + "%" : ""}</div></div>
      <div class="c-files">${n.dir ? fmtCount(n.files) : ""}</div>
      <div class="c-risk">${n.flags & F.VIRTUAL ? "" : badge(n.level, n.del_level)}</div>
      <div class="c-mtime">${fmtDate(n.mtime)}</div>
    </div>`;
  }
  inner.innerHTML = html;
}

async function toggle(i: number) {
  const r = state.rows[i];
  if (r?.kind !== "node" || !r.node.dir || !r.node.children) return;
  const id = r.node.id;
  if (state.expanded.has(id)) state.expanded.delete(id);
  else { await ensureKids(id); state.expanded.add(id); }
  await rebuild();
}

function setFocus(i: number) {
  state.focus = i;
  renderRows();
  const r = state.rows[i];
  if (r?.kind === "node") showDetail(r.node.id);
}

$("tree").addEventListener("scroll", () => renderRows());
window.addEventListener("resize", () => renderRows());
$("tree-inner").addEventListener("click", async (e) => {
  const t = e.target as HTMLElement;
  if (t.dataset.toggle) return toggle(Number(t.dataset.toggle));
  if (t.dataset.pick) return pick(Number(t.dataset.pick), (t as HTMLInputElement).checked);
  const row = t.closest<HTMLElement>(".row");
  if (!row) return;
  const i = Number(row.dataset.i);
  const r = state.rows[i];
  if (r.kind === "more") { await ensureKids(r.parent, true); return rebuild(); }
  setFocus(i);
});
$("tree-inner").addEventListener("dblclick", (e) => {
  const row = (e.target as HTMLElement).closest<HTMLElement>(".row");
  if (row && !(e.target as HTMLElement).dataset.pick) toggle(Number(row.dataset.i));
});
$("tree").tabIndex = 0;
$("tree").addEventListener("keydown", async (e) => {
  if (!visible() || !state.rows.length) return;
  const r = state.rows[state.focus];
  if (e.key === "ArrowDown") setFocus(Math.min(state.rows.length - 1, state.focus + 1));
  else if (e.key === "ArrowUp") setFocus(Math.max(0, state.focus - 1));
  else if (e.key === "ArrowRight" && r?.kind === "node" && !state.expanded.has(r.node.id)) await toggle(state.focus);
  else if (e.key === "ArrowLeft" && r?.kind === "node" && state.expanded.has(r.node.id)) await toggle(state.focus);
  else if (e.key === " " && r?.kind === "node") pick(state.focus, !state.basket.has(r.node.id));
  else return;
  e.preventDefault();
  const tree = $("tree");
  const y = state.focus * ROW_H;
  if (y < tree.scrollTop) tree.scrollTop = y;
  else if (y + ROW_H > tree.scrollTop + tree.clientHeight) tree.scrollTop = y + ROW_H - tree.clientHeight;
});

document.querySelectorAll<HTMLElement>(".tree-head [data-sort]").forEach((el) => {
  el.classList.toggle("on", el.dataset.sort === state.sort);
  el.addEventListener("click", async () => {
    state.sort = el.dataset.sort!;
    document.querySelectorAll(".tree-head [data-sort]").forEach((x) => x.classList.toggle("on", x === el));
    await refreshKids();
  });
});

/** 重新拉取所有已展开节点的子项（排序变化或删除后） */
async function refreshKids() {
  const info = await api.treeInfo();
  if (!info) return resetTree();
  state.root = info.root;
  const expanded = [...state.expanded];
  state.kids.clear();
  for (const id of expanded) {
    try { await ensureKids(id); } catch { state.expanded.delete(id); }
  }
  // 已删除的节点会从上级的子项里消失，展开集合里残留的不影响显示
  await rebuild();
  if (state.focus >= state.rows.length) state.focus = state.rows.length - 1;
}

// ---------- 详情 ----------
function switchTab(tab: string) {
  document.querySelectorAll<HTMLElement>(".tab").forEach((t) => t.classList.toggle("active", t.dataset.tab === tab));
  $("tab-detail").classList.toggle("hidden", tab !== "detail");
  $("tab-hits").classList.toggle("hidden", tab !== "hits");
}
document.querySelectorAll<HTMLElement>(".tab").forEach((t) => t.addEventListener("click", () => switchTab(t.dataset.tab!)));

async function showDetail(id: number) {
  const d = await api.detail(id);
  const n = d.node;
  const origin = d.origin === "protected" ? "系统保护名单" : d.origin === "rule" ? `清理规则 · ${esc(d.category)}` : "内置判断";
  const modeText = d.mode === "contents" ? "清理时只删除里面的内容，保留这个文件夹。"
    : d.mode === "empty_recycle_bin" ? "清理时会调用系统接口清空回收站。" : "";
  $("tab-detail").innerHTML = `
    <div style="display:flex;justify-content:space-between;align-items:center;gap:8px">
      <h3 style="margin:0;word-break:break-all">${esc(n.name)}</h3>${n.flags & F.VIRTUAL ? "" : badge(n.level, n.del_level)}
    </div>
    <div class="kv"><div class="k">位置</div><div class="v">${esc(d.path || "—")}</div></div>
    <div class="kv"><div class="k">大小</div><div class="v">${fmtSize(n.size)}${n.dir ? ` · ${fmtCount(n.files)} 个文件` : ""}${n.mtime ? ` · 修改于 ${fmtDate(n.mtime)}` : ""}</div></div>
    ${n.flags & F.VIRTUAL ? "" : `
    <div class="kv"><div class="k">判定依据</div><div class="v">${esc(d.rule_name)} <span class="muted">（${origin}${d.inherited ? "，继承自上级" : ""}）</span></div></div>
    <div class="kv"><div class="k">为什么是“${["安全", "需确认", "谨慎", "受保护"][n.level]}”</div><div class="note">${esc(d.note)}${modeText ? `<br/>${modeText}` : ""}</div>
      ${d.alt ? `<div class="alt">👉 ${esc(d.alt)}</div>` : ""}
      ${d.protected_inside ? `<div class="warn">里面包含受保护的项目，例如：${esc(d.protected_inside)}。整体不能删除，请展开后选择具体的部分。</div>` : ""}
      ${n.flags & F.PARTIAL ? `<div class="warn">快速扫描只统计了这里的一部分内容（已识别的可清理位置），显示的大小不是整个文件夹的大小。</div>` : ""}
      ${n.sub & 1 ? `<div class="warn">里面有没有权限读取的文件夹，实际大小可能更大。以管理员身份运行可以看到更多。</div>` : ""}
    </div>`}
    <div class="detail-actions">
      ${d.path ? `<button id="d-open">在资源管理器中显示</button>` : ""}
      ${n.flags & F.VIRTUAL || n.level >= 3 ? "" : `<button id="d-pick">${state.basket.has(id) ? "移出待删除清单" : "加入待删除清单"}</button>`}
    </div>`;
  document.getElementById("d-open")?.addEventListener("click", () => api.openInExplorer(id));
  document.getElementById("d-pick")?.addEventListener("click", () => {
    if (state.basket.has(id)) state.basket.delete(id); else state.basket.set(id, n);
    renderBasket(); renderRows(); showDetail(id);
  });
  switchTab("detail");
}

async function loadHits() {
  const hits = await api.hits();
  if (!hits.length) { $("tab-hits").innerHTML = `<p class="muted">没有找到可清理的项目。</p>`; return; }
  const safe = hits.filter((h) => h.node.del_level === 0);
  const total = hits.reduce((s, h) => s + h.node.size, 0);
  const safeTotal = safe.reduce((s, h) => s + h.node.size, 0);
  $("tab-hits").innerHTML = `
    <div class="hits-head"><div><b>${fmtSize(total)}</b> <span class="muted">可清理，其中安全的 ${fmtSize(safeTotal)}</span></div>
    <button id="pick-safe" class="primary" ${safe.length ? "" : "disabled"}>选中全部安全项</button></div>
    ${hits.map((h) => `<div class="hit">
      <input type="checkbox" data-hit="${h.node.id}" ${state.basket.has(h.node.id) ? "checked" : ""}/>
      <span class="hn" data-goto="${h.node.id}">${esc(h.node.rule ?? h.node.name)}</span>
      <span>${fmtSize(h.node.size)} ${badge(h.node.level, h.node.del_level)}</span>
      <span class="hp">${esc(h.path)}</span></div>`).join("")}`;
  $("tab-hits").querySelectorAll<HTMLInputElement>("[data-hit]").forEach((cb) => cb.addEventListener("change", () => {
    const h = hits.find((x) => x.node.id === Number(cb.dataset.hit))!;
    if (cb.checked) state.basket.set(h.node.id, h.node); else state.basket.delete(h.node.id);
    renderBasket(); renderRows();
  }));
  $("tab-hits").querySelectorAll<HTMLElement>("[data-goto]").forEach((el) => el.addEventListener("click", () => showDetail(Number(el.dataset.goto))));
  document.getElementById("pick-safe")?.addEventListener("click", () => {
    for (const h of safe) state.basket.set(h.node.id, h.node);
    renderBasket(); renderRows(); loadHits();
  });
}

// ---------- 待删除清单 ----------
function pick(i: number, on: boolean) {
  const r = state.rows[i];
  if (r?.kind !== "node") return;
  if (on && r.node.level >= 3) {
    toast("受保护的项目不能删除");
    renderRows();
    return;
  }
  if (on) state.basket.set(r.node.id, r.node); else state.basket.delete(r.node.id);
  renderBasket();
  renderRows();
}

function renderBasket() {
  const items = [...state.basket.values()];
  const total = items.reduce((s, n) => s + n.size, 0);
  const maxL = items.reduce((m, n) => Math.max(m, n.del_level), 0);
  const dl = state.treeDrive.slice(0, 2);
  $("basket-text").innerHTML = items.length
    ? `已选 ${dl} 中的 <b>${items.length}</b> 项，共 <b>${fmtSize(total)}</b> · 最高风险 ${badge(maxL)}`
      + (visible() ? "" : ` <span class="muted">（切换回 ${dl} 后可以删除）</span>`)
    : "勾选左侧的项目加入待删除清单";
  ($("btn-delete") as HTMLButtonElement).disabled = !items.length || state.scanning || !visible();
  $("btn-clear").classList.toggle("hidden", !items.length);
}

$("btn-clear").addEventListener("click", () => { state.basket.clear(); renderBasket(); renderRows(); loadHits(); });
$("btn-delete").addEventListener("click", () => {
  openDeleteFlow([...state.basket.keys()], async (out) => {
    state.basket.clear();
    renderBasket();
    await refreshKids();
    loadHits();
    loadDrives();
    if (out.failed.length === 0) $("tab-detail").innerHTML = `<p class="muted">点击左侧的一行查看详情。</p>`;
  });
});
$("btn-log").addEventListener("click", () => api.openLogDir());
$("btn-quick").addEventListener("click", () => startScan("quick"));
$("btn-full").addEventListener("click", () => startScan("full"));
$("btn-cancel").addEventListener("click", () => api.cancelScan());

let toastTimer: number | undefined;
function toast(msg: string) {
  const t = $("toast");
  t.textContent = msg;
  t.classList.remove("hidden");
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => t.classList.add("hidden"), 3500);
}

(async () => {
  loadAdmin();
  await loadDrives();
  await loadTree();
})();

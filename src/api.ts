import { invoke } from "@tauri-apps/api/core";
import { getLang, t, type Key } from "./i18n";

export interface NodeView {
  id: number; name: string; dir: boolean; size: number; files: number; mtime: number;
  level: number; max_level: number; del_level: number; children: number; flags: number; sub: number; rule: string | null;
}
export interface Drive { root: string; label: string; fs: string; kind: string; total: number; free: number; }
export interface Detail {
  node: NodeView; path: string; rule_id: string; rule_name: string; category: string;
  origin: "protected" | "rule" | "heuristic"; inherited: boolean; note: string; alt: string | null;
  mode: "self" | "contents" | "empty_recycle_bin"; protected_inside: string | null;
}
export interface Hit { node: NodeView; path: string; category: string; note: string; }
export interface PlanItem {
  id: number; name: string; path: string; size: number; level: number; rule_name: string;
  note: string; alt: string | null; mode: Detail["mode"]; blocked: string | null; strict: boolean;
}
export interface Plan {
  items: PlanItem[]; blocked: PlanItem[]; total: number; max_level: number;
  expected_text: string; typing_required: boolean; permanent_allowed: boolean;
}
export interface Outcome { freed: number; removed: number; failed: { path: string; reason: string }[]; }

export const F = { DIR: 1, INACCESSIBLE: 2, REPARSE: 4, CLOUD: 8, DELETED: 16, PARTIAL: 32, VIRTUAL: 64, INHERITED: 128 };

export const api = {
  setLang: (lang: string) => invoke<void>("set_lang", { lang }),
  drives: () => invoke<Drive[]>("list_drives"),
  isElevated: () => invoke<boolean>("is_elevated"),
  relaunchAsAdmin: () => invoke<void>("relaunch_as_admin"),
  startScan: (mode: "quick" | "full", root?: string) => invoke<void>("start_scan", { mode, root }),
  cancelScan: () => invoke<void>("cancel_scan"),
  treeInfo: () => invoke<{ root: NodeView; mode: "quick" | "full"; drive: string } | null>("get_tree_info"),
  children: (id: number, sort: string, offset: number, limit: number) =>
    invoke<{ items: NodeView[]; total: number }>("get_children", { id, sort, offset, limit }),
  detail: (id: number) => invoke<Detail>("get_detail", { id }),
  hits: () => invoke<Hit[]>("get_hits"),
  plan: (ids: number[]) => invoke<Plan>("plan_delete", { ids }),
  execute: (req: { ids: number[]; permanent: boolean; ack: boolean; typed: string }) =>
    invoke<Outcome>("execute_delete", { req }),
  openInExplorer: (id: number) => invoke<void>("open_in_explorer", { id }),
  openLogDir: () => invoke<void>("open_log_dir"),
};

export function levelName(level: number): string {
  return t(`L${Math.min(3, Math.max(0, level))}` as Key);
}

export function fmtSize(b: number): string {
  if (b < 1024) return `${b} B`;
  const u = ["KB", "MB", "GB", "TB"];
  let v = b / 1024, i = 0;
  while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
  return `${v >= 100 ? v.toFixed(0) : v >= 10 ? v.toFixed(1) : v.toFixed(2)} ${u[i]}`;
}

export function fmtCount(n: number): string {
  if (getLang() === "zh") {
    return n >= 10000 ? t("countWan", { n: (n / 10000).toFixed(n >= 100000 ? 0 : 1) }) : String(n);
  }
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n >= 10_000_000 ? 0 : 1)}M`;
  if (n >= 10_000) return `${(n / 1000).toFixed(0)}k`;
  return String(n);
}

export function fmtDate(ms: number): string {
  if (!ms) return "";
  const d = new Date(ms);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

export function badge(level: number, maxLevel = level): string {
  const mixed = maxLevel > level;
  const title = mixed ? t("levelMixed", { self: levelName(level), max: levelName(maxLevel) }) : levelName(maxLevel);
  return `<span class="badge l${maxLevel}${mixed ? " mixed" : ""}" title="${esc(title)}">${esc(levelName(maxLevel))}</span>`;
}

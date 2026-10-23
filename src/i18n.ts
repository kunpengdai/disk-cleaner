// 界面文字（中文 / English）。用 {name} 表示要替换的内容。

const zh = {
  appTitle: "磁盘清理",
  // 顶部
  quickScanOf: "快速扫描 {d}",
  fullScanOf: "全盘扫描 {d}",
  cancel: "取消",
  close: "关闭",
  admin: "管理员",
  normalRights: "普通权限",
  relaunchAdmin: "以管理员身份重启",
  relaunchAdminTip: "可以扫描和清理系统目录，例如 Windows 更新缓存",
  driveMeta: "可用 {free}，共 {total}",
  scannedQuick: "已快速扫描",
  scannedFull: "已全盘扫描",
  // 表头
  colName: "名称",
  colSize: "大小",
  colShare: "占上级",
  colFiles: "文件数",
  colRisk: "风险",
  colMtime: "修改时间",
  // 空白页
  notScanned: "{d} 还没有扫描",
  scanningNow: "{d} · 正在扫描…",
  driveUsage: "已用 {used}，可用 {free}，共 {total}",
  quickScan: "快速扫描",
  fullScan: "全盘扫描",
  quickScanDesc: "<b>快速扫描</b>：只检查已知的缓存、临时文件、回收站等位置，通常几秒钟。",
  fullScanDesc: "<b>全盘扫描</b>：统计这个磁盘里每个文件夹的大小，可能需要一两分钟。",
  otherDriveNote: "目前保存的是 {other} 的扫描结果，点击上方的该磁盘可以返回查看。扫描 {d} 后会替换它。",
  legendL0: "可直接清理",
  legendL1: "确认一次",
  legendL2: "多重确认，只进回收站",
  legendL3: "不能删除",
  // 扫描进度
  progressQuick: "正在检查 {d} 中已知的可清理位置…",
  progressFull: "正在扫描 {d} …",
  progressLine: "{sec} 秒 · {files} 个文件 · {dirs} 个文件夹 · {bytes}",
  progressDenied: " · {n} 处无权限",
  scanDone: "扫描完成，用时 {sec} 秒",
  scanCancelled: "已取消扫描",
  scanFailed: "扫描失败",
  // 树
  tagDenied: "无权限",
  tagLink: "链接",
  tagCloud: "仅云端",
  tagPartial: "部分统计",
  showMore: "显示更多（还有 {n} 项）",
  protectedNoDelete: "受保护，不能删除",
  // 详情
  tabDetail: "详情",
  tabHits: "可清理汇总",
  clickRowHint: "点击左侧的一行查看详情。",
  hitsAfterScan: "扫描 {d} 后，这里会列出可以清理的项目。",
  originProtected: "系统保护名单",
  originRule: "清理规则 · {cat}",
  originHeuristic: "内置判断",
  inheritedSuffix: "，继承自上级",
  modeContents: "清理时只删除里面的内容，保留这个文件夹。",
  modeRecycle: "清理时会调用系统接口清空回收站。",
  location: "位置",
  size: "大小",
  sizeFiles: " · {n} 个文件",
  modifiedOn: " · 修改于 {date}",
  basis: "判定依据",
  whyLevel: "为什么是“{level}”",
  protectedInside: "里面包含受保护的项目，例如：{p}。整体不能删除，请展开后选择具体的部分。",
  partialWarn: "快速扫描只统计了这里的一部分内容（已识别的可清理位置），显示的大小不是整个文件夹的大小。",
  deniedWarn: "里面有没有权限读取的文件夹，实际大小可能更大。以管理员身份运行可以看到更多。",
  showInExplorer: "在资源管理器中显示",
  addToList: "加入待删除清单",
  removeFromList: "移出待删除清单",
  // 汇总
  noHits: "没有找到可清理的项目。",
  hitsTotal: "<b>{total}</b> <span class=\"muted\">可清理，其中安全的 {safe}</span>",
  pickAllSafe: "选中全部安全项",
  // 待删除清单
  basketEmpty: "勾选左侧的项目加入待删除清单",
  basketText: "已选 {d} 中的 <b>{n}</b> 项，共 <b>{size}</b> · 最高风险 {badge}",
  basketOtherDrive: "（切换回 {d} 后可以删除）",
  deleteLog: "删除记录",
  clearList: "清空清单",
  deleteSelected: "删除所选…",
  cannotDeleteProtected: "受保护的项目不能删除",
  // 风险等级
  L0: "安全",
  L1: "需确认",
  L2: "谨慎",
  L3: "受保护",
  levelMixed: "本身：{self}；里面包含：{max}",
  // 删除弹窗
  dlgCannotTitle: "这些项目不能删除",
  dlgCautionTitle: "⚠ 删除谨慎项目",
  dlgConfirmTitle: "确认删除",
  dlgCleanTitle: "清理",
  dlgCannotSub: "所选项目都受保护，或者需要先完成全盘扫描。",
  dlgSub: "共 {n} 项，{size}。最高风险：{level}。",
  dlgSubCaution: "这些文件删除后可能导致软件出问题或丢失数据。",
  dlgModeContents: "（只清空里面的内容，保留文件夹）",
  dlgModeRecycle: "（清空回收站）",
  dlgUncheckTip: "取消勾选则不删除这一项",
  dlgNotDeleted: "以下 {n} 项不会被删除",
  dlgPermanent: "直接永久删除（缓存类文件放回收站没有意义）",
  dlgToRecycle: "将移到回收站，需要时可以从回收站还原。",
  dlgEmptyRecycleNote: "回收站清空后，里面的文件将无法再还原。",
  dlgEmptyRecycle: "清空回收站",
  dlgAboutTo: "你将要删除：",
  dlgAck: "我已了解：这些文件删除后，相关软件可能无法正常运行，个人数据可能丢失。文件会移到回收站，回收站清空后将无法恢复。",
  dlgNukeWarn: "如果 Windows 提示“文件太大无法放入回收站”，请在提示中选择“否”取消，以免永久删除。",
  dlgTypePrompt: "请输入 {text} 以确认删除：",
  dlgTypeNote: "文件会移到回收站。如果 Windows 提示“文件太大无法放入回收站”，请在提示中选择“否”取消，以免永久删除。",
  dlgDeleting: "正在删除…",
  dlgDeletePermanent: "永久删除",
  dlgMoveToRecycle: "移到回收站",
  dlgNextAfterRead: "我已看完，下一步",
  dlgBack: "上一步",
  dlgNext: "下一步",
  dlgWait: "请等待 {n} 秒",
  resPartial: "部分完成",
  resDone: "已完成",
  resFreed: "释放了 {size}",
  resRemoved: "删除了 {n} 项。重新扫描可以得到最准确的大小。",
  resFailed: "{n} 项未能完全删除",
  ok: "好的",
  // 数字
  countWan: "{n} 万",
};

type Dict = typeof zh;

const en: Dict = {
  appTitle: "Disk Cleaner",
  quickScanOf: "Quick scan {d}",
  fullScanOf: "Full scan {d}",
  cancel: "Cancel",
  close: "Close",
  admin: "Administrator",
  normalRights: "Standard user",
  relaunchAdmin: "Restart as administrator",
  relaunchAdminTip: "Lets you scan and clean system folders, such as the Windows Update cache",
  driveMeta: "{free} free of {total}",
  scannedQuick: "Quick scanned",
  scannedFull: "Fully scanned",
  colName: "Name",
  colSize: "Size",
  colShare: "Of parent",
  colFiles: "Files",
  colRisk: "Risk",
  colMtime: "Modified",
  notScanned: "{d} has not been scanned",
  scanningNow: "{d} · Scanning…",
  driveUsage: "{used} used, {free} free, {total} total",
  quickScan: "Quick scan",
  fullScan: "Full scan",
  quickScanDesc: "<b>Quick scan</b>: checks only known caches, temp files, the Recycle Bin and similar places. Usually takes a few seconds.",
  fullScanDesc: "<b>Full scan</b>: measures every folder on this drive. May take a minute or two.",
  otherDriveNote: "The saved results are for {other}. Click that drive above to view them again. Scanning {d} will replace them.",
  legendL0: "Clean directly",
  legendL1: "Confirm once",
  legendL2: "Several confirmations, Recycle Bin only",
  legendL3: "Cannot be deleted",
  progressQuick: "Checking known cleanable locations on {d}…",
  progressFull: "Scanning {d}…",
  progressLine: "{sec} s · {files} files · {dirs} folders · {bytes}",
  progressDenied: " · {n} not accessible",
  scanDone: "Scan finished in {sec} s",
  scanCancelled: "Scan cancelled",
  scanFailed: "Scan failed",
  tagDenied: "No access",
  tagLink: "Link",
  tagCloud: "Cloud only",
  tagPartial: "Partial",
  showMore: "Show more ({n} remaining)",
  protectedNoDelete: "Protected, cannot be deleted",
  tabDetail: "Details",
  tabHits: "Cleanable",
  clickRowHint: "Click a row on the left to see details.",
  hitsAfterScan: "Scan {d} to list items that can be cleaned here.",
  originProtected: "System protection list",
  originRule: "Cleanup rule · {cat}",
  originHeuristic: "Built-in check",
  inheritedSuffix: ", inherited from parent",
  modeContents: "Cleaning deletes only what is inside and keeps this folder.",
  modeRecycle: "Cleaning empties the Recycle Bin through Windows.",
  location: "Location",
  size: "Size",
  sizeFiles: " · {n} file(s)",
  modifiedOn: " · modified {date}",
  basis: "Classified as",
  whyLevel: "Why “{level}”",
  protectedInside: "Contains protected items, e.g. {p}. It cannot be deleted as a whole; expand it and pick specific parts.",
  partialWarn: "Quick scan only counted part of this folder (the recognized cleanable locations). The size shown is not the size of the whole folder.",
  deniedWarn: "Some folders inside could not be read, so the real size may be larger. Run as administrator to see more.",
  showInExplorer: "Show in File Explorer",
  addToList: "Add to delete list",
  removeFromList: "Remove from delete list",
  noHits: "Nothing to clean was found.",
  hitsTotal: "<b>{total}</b> <span class=\"muted\">can be cleaned, {safe} of it safe</span>",
  pickAllSafe: "Select all safe items",
  basketEmpty: "Tick items on the left to add them to the delete list",
  basketText: "<b>{n}</b> selected on {d} · <b>{size}</b> in total · highest risk {badge}",
  basketOtherDrive: "(switch back to {d} to delete)",
  deleteLog: "Delete log",
  clearList: "Clear list",
  deleteSelected: "Delete selected…",
  cannotDeleteProtected: "Protected items cannot be deleted",
  L0: "Safe",
  L1: "Confirm",
  L2: "Caution",
  L3: "Protected",
  levelMixed: "Itself: {self}; contains: {max}",
  dlgCannotTitle: "These items cannot be deleted",
  dlgCautionTitle: "⚠ Delete items marked Caution",
  dlgConfirmTitle: "Confirm delete",
  dlgCleanTitle: "Clean up",
  dlgCannotSub: "All selected items are protected, or need a full scan first.",
  dlgSub: "Items: {n}, {size}. Highest risk: {level}.",
  dlgSubCaution: "Deleting these files may break software or lose data.",
  dlgModeContents: "(empties the contents, keeps the folder)",
  dlgModeRecycle: "(empties the Recycle Bin)",
  dlgUncheckTip: "Untick to keep this item",
  dlgNotDeleted: "Will not be deleted ({n})",
  dlgPermanent: "Delete permanently (no point sending caches to the Recycle Bin)",
  dlgToRecycle: "Items will be moved to the Recycle Bin and can be restored from there.",
  dlgEmptyRecycleNote: "Once the Recycle Bin is emptied, its files can no longer be restored.",
  dlgEmptyRecycle: "Empty Recycle Bin",
  dlgAboutTo: "You are about to delete:",
  dlgAck: "I understand: after deleting these files, related software may stop working and personal data may be lost. Files go to the Recycle Bin and cannot be recovered once it is emptied.",
  dlgNukeWarn: "If Windows says a file is too big for the Recycle Bin, choose “No” to cancel so it is not deleted permanently.",
  dlgTypePrompt: "Type {text} to confirm:",
  dlgTypeNote: "Files go to the Recycle Bin. If Windows says a file is too big for the Recycle Bin, choose “No” to cancel so it is not deleted permanently.",
  dlgDeleting: "Deleting…",
  dlgDeletePermanent: "Delete permanently",
  dlgMoveToRecycle: "Move to Recycle Bin",
  dlgNextAfterRead: "I've read this, next",
  dlgBack: "Back",
  dlgNext: "Next",
  dlgWait: "Wait {n} s",
  resPartial: "Partly done",
  resDone: "Done",
  resFreed: "Freed {size}",
  resRemoved: "Items deleted: {n}. Scan again for the most accurate sizes.",
  resFailed: "Not fully deleted ({n})",
  ok: "OK",
  countWan: "",
};

export type Lang = "zh" | "en";
export type Key = keyof Dict;
const dicts: Record<Lang, Dict> = { zh, en };
const STORE_KEY = "disk-cleaner.lang";

function detect(): Lang {
  try {
    const saved = localStorage.getItem(STORE_KEY);
    if (saved === "zh" || saved === "en") return saved;
  } catch { /* 读不到就按系统语言 */ }
  return navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
}

let lang: Lang = detect();

export function getLang(): Lang {
  return lang;
}

export function setLang(l: Lang) {
  lang = l;
  try { localStorage.setItem(STORE_KEY, l); } catch { /* 记不住也没关系 */ }
  document.documentElement.lang = l === "zh" ? "zh-CN" : "en";
}

export function t(key: Key, p: Record<string, string | number> = {}): string {
  return dicts[lang][key].replace(/\{(\w+)\}/g, (_, k) => String(p[k] ?? ""));
}

/** 页面里写死的文字：data-i18n 设置文字，data-i18n-title 设置提示 */
export function applyStatic() {
  document.querySelectorAll<HTMLElement>("[data-i18n]").forEach((el) => { el.textContent = t(el.dataset.i18n as Key); });
  document.querySelectorAll<HTMLElement>("[data-i18n-title]").forEach((el) => { el.title = t(el.dataset.i18nTitle as Key); });
  document.title = t("appTitle");
}

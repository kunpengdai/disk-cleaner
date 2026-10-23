import { api, badge, esc, fmtSize, levelName, type Outcome, type Plan, type PlanItem } from "./api";
import { t } from "./i18n";

const modal = document.getElementById("modal")!;
const dialog = document.getElementById("dialog")!;
const COUNTDOWN = 5;

function close() {
  modal.classList.add("hidden");
  dialog.innerHTML = "";
}

function modeText(it: PlanItem): string {
  if (it.mode === "contents") return t("dlgModeContents");
  if (it.mode === "empty_recycle_bin") return t("dlgModeRecycle");
  return "";
}

function itemHtml(it: PlanItem, checkable: boolean): string {
  return `<div class="pitem${it.blocked ? " blocked" : ""}">
    ${checkable ? `<input type="checkbox" checked data-id="${it.id}" title="${esc(t("dlgUncheckTip"))}" />` : "<span></span>"}
    <div class="pn" title="${esc(it.name)}">${esc(it.name)} <span class="muted">${modeText(it)}</span></div>
    <div>${fmtSize(it.size)}</div>
    <div>${badge(it.level)}</div>
    <div class="pp">${esc(it.path)}</div>
    ${it.blocked
      ? `<div class="pnote">⛔ ${esc(it.blocked)}${it.alt ? `<br/>👉 ${esc(it.alt)}` : ""}</div>`
      : `<div class="pnote">${esc(it.note)}${it.alt ? `<br/>👉 ${esc(it.alt)}` : ""}</div>`}
  </div>`;
}

/**
 * 打开删除确认流程。
 * - 安全 / 需确认：一步确认
 * - 谨慎（只是规则未识别）：两步：看影响 → 勾选已了解
 * - 谨慎（规则明确识别，如软件数据、个人文件）：三步：看影响 → 勾选已了解 → 输入名称并等待倒计时
 */
export async function openDeleteFlow(initialIds: number[], onDone: (o: Outcome) => void) {
  let ids = [...initialIds];
  let plan: Plan = await api.plan(ids);
  let step = 1;
  let ack = false;
  let typed = "";
  let permanent = plan.permanent_allowed;
  let remain = COUNTDOWN;
  let timer: number | undefined;
  let running = false;

  const stopTimer = () => { if (timer) { clearInterval(timer); timer = undefined; } };

  async function replan() {
    plan = await api.plan(ids);
    step = 1; ack = false; typed = ""; stopTimer();
    permanent = plan.permanent_allowed;
    render();
  }

  function render() {
    const L = plan.max_level;
    const multi = L >= 2;
    const steps = plan.typing_required ? 3 : 2;
    const allowed = plan.items;
    const title = allowed.length === 0 ? t("dlgCannotTitle")
      : L >= 2 ? t("dlgCautionTitle") : L === 1 ? t("dlgConfirmTitle") : t("dlgCleanTitle");
    const sub = allowed.length === 0 ? t("dlgCannotSub")
      : t("dlgSub", { n: allowed.length, size: fmtSize(plan.total), level: levelName(L) })
        + (L >= 2 ? " " + t("dlgSubCaution") : "");
    const recycleBtn = running ? t("dlgDeleting") : t("dlgMoveToRecycle");
    // 只是清空回收站时，不能说“可以从回收站还原”
    const onlyEmptyBin = allowed.length > 0 && allowed.every((it) => it.mode === "empty_recycle_bin");

    let body = "";
    if (!multi || step === 1) {
      body += allowed.map((it) => itemHtml(it, allowed.length > 1 || plan.blocked.length > 0)).join("");
      if (plan.blocked.length) {
        body += `<div class="section-title">${esc(t("dlgNotDeleted", { n: plan.blocked.length }))}</div>`;
        body += plan.blocked.map((it) => itemHtml(it, false)).join("");
      }
      if (!multi && allowed.length) {
        body += `<div style="margin:12px 0">${plan.permanent_allowed
          ? `<label class="opt"><input type="checkbox" id="perm" ${permanent ? "checked" : ""}/> ${esc(t("dlgPermanent"))}</label>`
          : `<span class="muted">${esc(t(onlyEmptyBin ? "dlgEmptyRecycleNote" : "dlgToRecycle"))}</span>`}</div>`;
      }
    } else if (step === 2) {
      body += `<p>${esc(t("dlgAboutTo"))}</p><ul>${allowed.map((it) => `<li><b>${esc(it.name)}</b> <span class="muted">${fmtSize(it.size)} · ${esc(it.rule_name)}</span></li>`).join("")}</ul>
        <label class="ack"><input type="checkbox" id="ack" ${ack ? "checked" : ""}/>
        <span>${esc(t("dlgAck"))}</span></label>
        ${steps === 2 ? `<p class="muted">${esc(t("dlgNukeWarn"))}</p>` : ""}`;
    } else {
      body += `<p>${t("dlgTypePrompt", { text: `<span class="expect">${esc(plan.expected_text)}</span>` })}</p>
        <input class="typed" id="typed" autocomplete="off" spellcheck="false" value="${esc(typed)}" />
        <p class="muted">${esc(t("dlgTypeNote"))}</p>`;
    }

    const canGo = allowed.length > 0 && !running;
    let foot = `<div class="grow">${allowed.length ? fmtSize(plan.total) : ""}</div><button id="cancel">${esc(t("cancel"))}</button>`;
    if (!allowed.length) {
      foot = `<div class="grow"></div><button id="cancel">${esc(t("close"))}</button>`;
    } else if (!multi) {
      const label = running ? t("dlgDeleting") : permanent ? t("dlgDeletePermanent")
        : onlyEmptyBin ? t("dlgEmptyRecycle") : t("dlgMoveToRecycle");
      foot += `<button id="go" class="${L === 0 ? "primary" : "danger"}" ${canGo ? "" : "disabled"}>${esc(label)}</button>`;
    } else if (step === 1) {
      foot += `<button id="next" class="primary">${esc(t("dlgNextAfterRead"))}</button>`;
    } else if (step === 2 && steps === 3) {
      foot += `<button id="back">${esc(t("dlgBack"))}</button><button id="next" class="primary" ${ack ? "" : "disabled"}>${esc(t("dlgNext"))}</button>`;
    } else if (step === 2) {
      foot += `<button id="back">${esc(t("dlgBack"))}</button><button id="go" class="danger" ${ack && canGo ? "" : "disabled"}>${esc(recycleBtn)}</button>`;
    } else {
      const ok = typed.trim() === plan.expected_text && remain <= 0;
      const label = running ? t("dlgDeleting") : remain > 0 ? t("dlgWait", { n: remain }) : t("dlgMoveToRecycle");
      foot += `<button id="back">${esc(t("dlgBack"))}</button><button id="go" class="danger" ${ok && canGo ? "" : "disabled"}>${esc(label)}</button>`;
    }

    dialog.innerHTML = `<h3>${esc(title)}</h3><div class="sub">${esc(sub)}</div>
      ${multi && allowed.length ? `<div class="steps">${Array.from({ length: steps }, (_, k) => k + 1).map((s) => `<span class="${s <= step ? "on" : ""}"></span>`).join("")}</div>` : ""}
      <div class="body">${body}</div><div class="foot">${foot}</div>`;
    modal.classList.remove("hidden");
    bind();
  }

  function bind() {
    dialog.querySelector("#cancel")?.addEventListener("click", () => { if (!running) { stopTimer(); close(); } });
    dialog.querySelectorAll<HTMLInputElement>(".pitem input[data-id]").forEach((cb) =>
      cb.addEventListener("change", () => {
        ids = ids.filter((i) => i !== Number(cb.dataset.id));
        replan();
      }));
    dialog.querySelector<HTMLInputElement>("#perm")?.addEventListener("change", (e) => {
      permanent = (e.target as HTMLInputElement).checked; render();
    });
    dialog.querySelector<HTMLInputElement>("#ack")?.addEventListener("change", (e) => {
      ack = (e.target as HTMLInputElement).checked; render();
    });
    const input = dialog.querySelector<HTMLInputElement>("#typed");
    if (input) {
      input.focus();
      input.setSelectionRange(input.value.length, input.value.length);
      input.addEventListener("paste", (e) => e.preventDefault()); // 必须手动输入
      input.addEventListener("input", () => { typed = input.value; render(); });
    }
    dialog.querySelector("#back")?.addEventListener("click", () => { step--; stopTimer(); render(); });
    dialog.querySelector("#next")?.addEventListener("click", () => {
      step++;
      if (step === 3) {
        remain = COUNTDOWN;
        stopTimer();
        timer = window.setInterval(() => { remain--; if (remain <= 0) stopTimer(); render(); }, 1000);
      }
      render();
    });
    dialog.querySelector("#go")?.addEventListener("click", execute);
  }

  async function execute() {
    running = true; render();
    try {
      const out = await api.execute({ ids: plan.items.map((i) => i.id), permanent: permanent && plan.permanent_allowed, ack, typed });
      stopTimer();
      showResult(out);
      onDone(out);
    } catch (e) {
      running = false; render();
      alert(String(e));
    }
  }

  render();
}

function showResult(o: Outcome) {
  dialog.innerHTML = `<h3>${esc(o.failed.length ? t("resPartial") : t("resDone"))}</h3>
    <div class="body">
      <div class="result-big">${esc(t("resFreed", { size: fmtSize(o.freed) }))}</div>
      <p class="muted">${esc(t("resRemoved", { n: o.removed }))}</p>
      ${o.failed.length ? `<div class="section-title">${esc(t("resFailed", { n: o.failed.length }))}</div>` + o.failed.map((f) =>
        `<div class="pitem"><span></span><div class="pn">${esc(f.path)}</div><div></div><div></div><div class="pnote">${esc(f.reason)}</div></div>`).join("") : ""}
    </div>
    <div class="foot"><div class="grow"></div><button id="ok" class="primary">${esc(t("ok"))}</button></div>`;
  dialog.querySelector("#ok")!.addEventListener("click", close);
}

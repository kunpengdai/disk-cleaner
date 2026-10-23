import { api, badge, esc, fmtSize, LEVEL_NAMES, type Outcome, type Plan, type PlanItem } from "./api";

const modal = document.getElementById("modal")!;
const dialog = document.getElementById("dialog")!;
const COUNTDOWN = 5;

function close() {
  modal.classList.add("hidden");
  dialog.innerHTML = "";
}

function modeText(it: PlanItem): string {
  if (it.mode === "contents") return "（只清空里面的内容，保留文件夹）";
  if (it.mode === "empty_recycle_bin") return "（清空回收站）";
  return "";
}

function itemHtml(it: PlanItem, checkable: boolean): string {
  return `<div class="pitem${it.blocked ? " blocked" : ""}">
    ${checkable ? `<input type="checkbox" checked data-id="${it.id}" title="取消勾选则不删除这一项" />` : "<span></span>"}
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
    const title = allowed.length === 0 ? "这些项目不能删除"
      : L >= 2 ? "⚠ 删除谨慎项目" : L === 1 ? "确认删除" : "清理";
    const sub = allowed.length === 0 ? "所选项目都受保护，或者需要先完成全盘扫描。"
      : `共 ${allowed.length} 项，${fmtSize(plan.total)}。最高风险：${LEVEL_NAMES[L]}。`
        + (L >= 2 ? "这些文件删除后可能导致软件出问题或丢失数据。" : "");

    let body = "";
    if (!multi || step === 1) {
      body += allowed.map((it) => itemHtml(it, allowed.length > 1 || plan.blocked.length > 0)).join("");
      if (plan.blocked.length) {
        body += `<div class="section-title">以下 ${plan.blocked.length} 项不会被删除</div>`;
        body += plan.blocked.map((it) => itemHtml(it, false)).join("");
      }
      if (!multi && allowed.length) {
        body += `<div style="margin:12px 0">${plan.permanent_allowed
          ? `<label class="opt"><input type="checkbox" id="perm" ${permanent ? "checked" : ""}/> 直接永久删除（缓存类文件放回收站没有意义）</label>`
          : `<span class="muted">将移到回收站，需要时可以从回收站还原。</span>`}</div>`;
      }
    } else if (step === 2) {
      body += `<p>你将要删除：</p><ul>${allowed.map((it) => `<li><b>${esc(it.name)}</b> <span class="muted">${fmtSize(it.size)} · ${esc(it.rule_name)}</span></li>`).join("")}</ul>
        <label class="ack"><input type="checkbox" id="ack" ${ack ? "checked" : ""}/>
        <span>我已了解：这些文件删除后，相关软件可能无法正常运行，个人数据可能丢失。文件会移到回收站，回收站清空后将无法恢复。</span></label>
        ${steps === 2 ? `<p class="muted">如果 Windows 提示“文件太大无法放入回收站”，请在提示中选择“否”取消，以免永久删除。</p>` : ""}`;
    } else {
      body += `<p>请输入 <span class="expect">${esc(plan.expected_text)}</span> 以确认删除：</p>
        <input class="typed" id="typed" autocomplete="off" spellcheck="false" value="${esc(typed)}" />
        <p class="muted">文件会移到回收站。如果 Windows 提示“文件太大无法放入回收站”，请在提示中选择“否”取消，以免永久删除。</p>`;
    }

    const canGo = allowed.length > 0 && !running;
    let foot = `<div class="grow">${allowed.length ? fmtSize(plan.total) : ""}</div><button id="cancel">取消</button>`;
    if (!allowed.length) {
      foot = `<div class="grow"></div><button id="cancel">关闭</button>`;
    } else if (!multi) {
      foot += `<button id="go" class="${L === 0 ? "primary" : "danger"}" ${canGo ? "" : "disabled"}>${running ? "正在删除…" : permanent ? "永久删除" : "移到回收站"}</button>`;
    } else if (step === 1) {
      foot += `<button id="next" class="primary">我已看完，下一步</button>`;
    } else if (step === 2 && steps === 3) {
      foot += `<button id="back">上一步</button><button id="next" class="primary" ${ack ? "" : "disabled"}>下一步</button>`;
    } else if (step === 2) {
      foot += `<button id="back">上一步</button><button id="go" class="danger" ${ack && canGo ? "" : "disabled"}>${running ? "正在删除…" : "移到回收站"}</button>`;
    } else {
      const ok = typed.trim() === plan.expected_text && remain <= 0;
      foot += `<button id="back">上一步</button><button id="go" class="danger" ${ok && canGo ? "" : "disabled"}>${running ? "正在删除…" : remain > 0 ? `请等待 ${remain} 秒` : "移到回收站"}</button>`;
    }

    dialog.innerHTML = `<h3>${title}</h3><div class="sub">${sub}</div>
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
  dialog.innerHTML = `<h3>${o.failed.length ? "部分完成" : "已完成"}</h3>
    <div class="body">
      <div class="result-big">释放了 ${fmtSize(o.freed)}</div>
      <p class="muted">删除了 ${o.removed} 项。重新扫描可以得到最准确的大小。</p>
      ${o.failed.length ? `<div class="section-title">${o.failed.length} 项未能完全删除</div>` + o.failed.map((f) =>
        `<div class="pitem"><span></span><div class="pn">${esc(f.path)}</div><div></div><div></div><div class="pnote">${esc(f.reason)}</div></div>`).join("") : ""}
    </div>
    <div class="foot"><div class="grow"></div><button id="ok" class="primary">好的</button></div>`;
  dialog.querySelector("#ok")!.addEventListener("click", close);
}

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";

const $ = (id) => document.getElementById(id);
const appWindow = getCurrentWindow();

const standardInput = $("standardInput");
const btnQuery = $("btnQuery");
const validityResult = $("validityResult");
const cnasResult = $("cnasResult");
const cmaResult = $("cmaResult");
const cmaApiResult = $("cmaApiResult");

// In-memory config (loaded from persisted config.json on startup)
let config = { cma_url: "", samr_url: "" };

function setLoading(loading) {
  btnQuery.disabled = loading;
  ["validitySpinner", "cnasSpinner", "cmaSpinner", "cmaApiSpinner"].forEach((id) => {
    $(id).classList.toggle("hidden", !loading);
  });
}

function setPlaceholder(el, text) {
  el.textContent = "";
  const span = document.createElement("span");
  span.className = "color-gray";
  span.textContent = text;
  el.appendChild(span);
}

function resetResults() {
  setPlaceholder(validityResult, "查询中...");
  setPlaceholder(cnasResult, "查询中...");
  setPlaceholder(cmaResult, "查询中...");
  setPlaceholder(cmaApiResult, "查询中...");
}

function statusColor(status) {
  switch (status) {
    case "exact": return "color-green";
    case "partial": return "color-yellow";
    case "nomatch":
    case "error": return "color-red";
    default: return "color-gray";
  }
}

function renderValidity(result) {
  validityResult.textContent = "";
  result.lines.forEach((line, i) => {
    if (i > 0) validityResult.appendChild(document.createElement("br"));
    const span = document.createElement("span");
    span.className = `color-${line.color}`;
    span.textContent = line.text;
    validityResult.appendChild(span);
  });
}

function renderMatch(el, result) {
  el.textContent = "";
  const span = document.createElement("span");
  span.className = statusColor(result.status);
  span.textContent = result.message;
  el.appendChild(span);
}

function renderError(el) {
  el.textContent = "";
  const span = document.createElement("span");
  span.className = "color-red";
  span.textContent = "查询异常";
  el.appendChild(span);
}

async function doQuery() {
  const input = standardInput.value.trim();
  if (!input) {
    setPlaceholder(validityResult, "请先输入标准号或标准名称");
    return;
  }

  setLoading(true);
  resetResults();

  const cmaUrl = config.cma_url;
  const samrUrl = config.samr_url;

  const [validity, cnas, cma, cmaApi] = await Promise.allSettled([
    invoke("query_validity", { stdCode: input, samrUrl }),
    invoke("query_cnas", { stdCode: input }),
    invoke("query_cma_file", { stdCode: input }),
    invoke("query_cma_api", { stdCode: input, baseUrl: cmaUrl }),
  ]);

  if (validity.status === "fulfilled") renderValidity(validity.value);
  else renderError(validityResult);

  if (cnas.status === "fulfilled") renderMatch(cnasResult, cnas.value);
  else renderError(cnasResult);

  if (cma.status === "fulfilled") renderMatch(cmaResult, cma.value);
  else renderError(cmaResult);

  if (cmaApi.status === "fulfilled") renderMatch(cmaApiResult, cmaApi.value);
  else renderError(cmaApiResult);

  setLoading(false);
}

btnQuery.addEventListener("click", doQuery);
standardInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") doQuery();
});

$("btnCnasFile").addEventListener("click", async () => {
  const path = await open({
    filters: [{ name: "附表文件", extensions: ["pdf", "xlsx", "xls"] }],
  });
  if (!path) return;
  try {
    const count = await invoke("load_cnas_file", { path });
    $("cnasFileName").value = path.split(/[/\\]/).pop();
    $("cnasFileName").title = `${path}（${count} 个标准）`;
  } catch (e) {
    alert(`解析CNAS附表失败：${e}`);
  }
});

$("btnCmaFile").addEventListener("click", async () => {
  const path = await open({
    filters: [{ name: "附表文件", extensions: ["pdf", "xlsx", "xls"] }],
  });
  if (!path) return;
  try {
    const count = await invoke("load_cma_file", { path });
    $("cmaFileName").value = path.split(/[/\\]/).pop();
    $("cmaFileName").title = `${path}（${count} 个标准）`;
  } catch (e) {
    alert(`解析CMA附表失败：${e}`);
  }
});

// ===== Window controls (custom title bar) =====
$("btnMinimize").addEventListener("click", () => appWindow.minimize());
$("btnMaximize").addEventListener("click", () => appWindow.toggleMaximize());
$("btnClose").addEventListener("click", () => appWindow.close());

// ===== Settings panel =====
const settingsModal = $("settingsModal");
const settingsStatus = $("settingsStatus");

function openSettings() {
  $("cmaUrl").value = config.cma_url;
  $("samrUrl").value = config.samr_url;
  settingsStatus.textContent = "";
  settingsModal.classList.remove("hidden");
}

function closeSettings() {
  settingsModal.classList.add("hidden");
}

$("btnSettings").addEventListener("click", openSettings);
$("btnCloseSettings").addEventListener("click", closeSettings);
settingsModal.addEventListener("click", (e) => {
  if (e.target === settingsModal) closeSettings();
});
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && !settingsModal.classList.contains("hidden")) closeSettings();
});

$("btnSaveSettings").addEventListener("click", async () => {
  const cmaUrl = $("cmaUrl").value.trim();
  const samrUrl = $("samrUrl").value.trim();
  try {
    await invoke("save_config", { cmaUrl, samrUrl });
    config.cma_url = cmaUrl;
    config.samr_url = samrUrl;
    settingsStatus.textContent = "已保存";
  } catch (e) {
    settingsStatus.textContent = `保存失败：${e}`;
  }
});

$("btnCheckUpdate").addEventListener("click", async () => {
  settingsStatus.textContent = "正在检查更新...";
  try {
    const msg = await invoke("check_update");
    settingsStatus.textContent = msg;
  } catch (e) {
    settingsStatus.textContent = `检查更新失败：${e}`;
  }
});

// ===== Startup: load persisted config =====
(async () => {
  try {
    config = await invoke("get_config");
  } catch (e) {
    console.error("加载配置失败", e);
  }
})();

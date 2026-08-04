import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";

const $ = (id) => document.getElementById(id);
const appWindow = getCurrentWindow();

const standardInput = $("standardInput");
const btnQuery = $("btnQuery");
const resultsBody = $("resultsBody");
const resultsScroll = $("resultsScroll");

// Current row cells: { validity, cmaApi, cnas, cmaFile }
let curCells = null;

function createCells() {
  const mk = (cls) => {
    const d = document.createElement("div");
    d.className = `result-cell ${cls}`;
    resultsBody.appendChild(d);
    return d;
  };
  curCells = {
    validity: mk("cell-validity"),
    cmaApi: mk("cell-cmaapi"),
    cnas: mk("cell-cnas"),
    cmaFile: mk("cell-cmafile"),
  };
  return curCells;
}

function clearResultsArea() {
  resultsBody.textContent = "";
  curCells = null;
}

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
  clearResultsArea();
  const c = createCells();
  setPlaceholder(c.validity, "查询中...");
  setPlaceholder(c.cmaApi, "查询中...");
  setPlaceholder(c.cnas, "查询中...");
  setPlaceholder(c.cmaFile, "查询中...");
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

function renderValidityTo(el, result) {
  el.textContent = "";
  result.lines.forEach((line, i) => {
    if (i > 0) el.appendChild(document.createElement("br"));
    const span = document.createElement("span");
    span.className = `color-${line.color}`;
    span.textContent = line.text;
    el.appendChild(span);
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
    clearResultsArea();
    setPlaceholder(createCells().validity, "请先输入标准号或标准名称");
    return;
  }

  setLoading(true);
  resetResults();
  $("btnOpenSamr").classList.remove("hidden");
  $("btnOpenCma").classList.remove("hidden");

  const cmaUrl = config.cma_url;
  const samrUrl = config.samr_url;

  const [validity, cnas, cma, cmaApi] = await Promise.allSettled([
    invoke("query_validity", { stdCode: input, samrUrl }),
    invoke("query_cnas", { stdCode: input }),
    invoke("query_cma_file", { stdCode: input }),
    invoke("query_cma_api", { stdCode: input, baseUrl: cmaUrl }),
  ]);

  const c = curCells;
  if (validity.status === "fulfilled") renderValidityTo(c.validity, validity.value);
  else renderError(c.validity);

  if (cmaApi.status === "fulfilled") renderMatch(c.cmaApi, cmaApi.value);
  else renderError(c.cmaApi);

  if (cnas.status === "fulfilled") renderMatch(c.cnas, cnas.value);
  else renderError(c.cnas);

  if (cma.status === "fulfilled") renderMatch(c.cmaFile, cma.value);
  else renderError(c.cmaFile);

  setLoading(false);
}

btnQuery.addEventListener("click", doQuery);
standardInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") doQuery();
});

// ===== Loaded appendix files (multi-file) =====
function renderFileList(listEl, files, kind) {
  listEl.textContent = "";
  if (!files || files.length === 0) {
    const empty = document.createElement("span");
    empty.className = "file-list-empty";
    empty.textContent = "未加载";
    listEl.appendChild(empty);
    return;
  }
  files.forEach((f, idx) => {
    const chip = document.createElement("span");
    chip.className = "file-chip";
    chip.title = f.name;

    const label = document.createElement("span");
    label.className = "file-chip-name";
    label.textContent = `${f.name} (${f.count})`;

    const rm = document.createElement("button");
    rm.className = "file-chip-remove";
    rm.textContent = "×";
    rm.title = "移除";
    rm.addEventListener("click", async () => {
      const cmd = kind === "cnas" ? "remove_cnas_file" : "remove_cma_file";
      const updated = await invoke(cmd, { index: idx });
      renderFileList(listEl, updated, kind);
    });

    chip.appendChild(label);
    chip.appendChild(rm);
    listEl.appendChild(chip);
  });
}

async function loadFiles(kind) {
  const paths = await open({
    multiple: true,
    filters: [{ name: "附表文件", extensions: ["pdf", "xlsx", "xls"] }],
  });
  if (!paths) return;

  const cmd = kind === "cnas" ? "load_cnas_file" : "load_cma_file";
  const listEl = $(kind === "cnas" ? "cnasFileList" : "cmaFileList");
  let latest = null;
  for (const path of paths) {
    try {
      latest = await invoke(cmd, { path });
    } catch (e) {
      alert(`解析附表失败：${path}\n${e}`);
    }
  }
  if (latest !== null) renderFileList(listEl, latest, kind);
}

$("btnCnasFile").addEventListener("click", () => loadFiles("cnas"));
$("btnCmaFile").addEventListener("click", () => loadFiles("cma"));

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
  $("updateProgress").classList.add("hidden");
  const btnCheck = $("btnCheckUpdate");
  btnCheck.textContent = "检查软件更新";
  btnCheck.disabled = false;
  btnCheck.dataset.state = "check";
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

$("btnOpenSamr").addEventListener("click", () => {
  const code = standardInput.value.trim().replace(/\s+/g, "");
  if (!code) return;
  const url = `https://std.samr.gov.cn/search/std?q=${encodeURIComponent(code)}`;
  invoke("open_url", { url });
});

$("btnOpenCma").addEventListener("click", () => {
  const code = standardInput.value.trim().replace(/\s+/g, "");
  const baseUrl = config.cma_url || "https://cma.caqit.org.cn";
  const base = baseUrl.replace(/\/+$/, "");
  const url = code
    ? `${base}/#/data?standardCode=${encodeURIComponent(code)}`
    : `${base}/#/data`;
  invoke("open_url", { url });
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
  const btn = $("btnCheckUpdate");

  if (btn.dataset.state === "restart") {
    invoke("apply_update");
    return;
  }

  const status = settingsStatus;
  const progress = $("updateProgress");
  const fill = $("progressFill");
  const text = $("progressText");

  status.textContent = "正在检查更新...";
  progress.classList.add("hidden");
  btn.disabled = true;

  try {
    const info = await invoke("check_update");
    if (!info.has_update) {
      status.textContent = info.message;
      btn.disabled = false;
      return;
    }

    status.textContent = info.message + "，正在下载...";
    progress.classList.remove("hidden");
    fill.style.width = "0%";
    text.textContent = "0%";

    const unlisten = await listen("update-progress", (event) => {
      const p = event.payload;
      const pct = Math.round(p.percent);
      fill.style.width = pct + "%";
      text.textContent = pct + "%";
    });

    await invoke("download_update", { url: info.url });
    unlisten();

    progress.classList.add("hidden");
    status.textContent = `v${info.version} 下载完成`;
    btn.textContent = "重启使用新版本";
    btn.dataset.state = "restart";
    btn.disabled = false;
  } catch (e) {
    progress.classList.add("hidden");
    status.textContent = `更新失败：${e}`;
    btn.disabled = false;
  }
});

// ===== Startup: load persisted config + restore file state =====
(async () => {
  try {
    config = await invoke("get_config");
  } catch (e) {
    console.error("加载配置失败", e);
  }
  try {
    const [cnasFiles, cmaFiles] = await invoke("restore_state");
    renderFileList($("cnasFileList"), cnasFiles, "cnas");
    renderFileList($("cmaFileList"), cmaFiles, "cma");
  } catch (e) {
    console.error("恢复状态失败", e);
  }
})();

// ===== Browse parsed standards =====
const browseModal = $("browseModal");
const browseList = $("browseList");
const browseSearch = $("browseSearch");
let allStandards = [];

function stdPrefix(code) {
  const m = code.match(/^([A-Za-z]+[/]?[A-Za-z]*)/);
  return m ? m[1].toUpperCase() : "其他";
}

function stdNumber(code) {
  const m = code.match(/[A-Za-z]+[/]?[A-Za-z]*\s*([0-9]+(?:[.\-][0-9]+)*)/);
  if (!m) return [0];
  return m[1].split(/[.\-]/).map(Number);
}

function compareNumbers(a, b) {
  const na = stdNumber(a.code);
  const nb = stdNumber(b.code);
  for (let i = 0; i < Math.max(na.length, nb.length); i++) {
    const va = na[i] || 0;
    const vb = nb[i] || 0;
    if (va !== vb) return va - vb;
  }
  return 0;
}

function renderBrowseList(filter) {
  browseList.textContent = "";
  const q = (filter || "").toLowerCase();
  const filtered = q
    ? allStandards.filter(
        (s) => s.code.toLowerCase().includes(q) || s.name.toLowerCase().includes(q)
      )
    : allStandards;

  if (filtered.length === 0) {
    const empty = document.createElement("div");
    empty.className = "browse-empty";
    empty.textContent = q ? "无匹配结果" : "暂无已解析的标准，请先加载附表文件";
    browseList.appendChild(empty);
    return;
  }

  const groups = {};
  for (const s of filtered) {
    const prefix = stdPrefix(s.code);
    if (!groups[prefix]) groups[prefix] = [];
    groups[prefix].push(s);
  }

  const sortedPrefixes = Object.keys(groups).sort();
  for (const prefix of sortedPrefixes) {
    const items = groups[prefix].sort(compareNumbers);

    const details = document.createElement("details");
    details.className = "browse-group";
    details.open = true;

    const summary = document.createElement("summary");
    summary.className = "browse-group-header";
    summary.textContent = `${prefix} (${items.length})`;
    details.appendChild(summary);

    for (const item of items) {
      const row = document.createElement("div");
      row.className = "browse-item";
      if (item.page && item.source_path.toLowerCase().endsWith(".pdf")) {
        row.classList.add("browse-item-clickable");
        row.title = `点击在浏览器中打开 PDF 第 ${item.page} 页`;
        row.addEventListener("click", () => {
          invoke("open_pdf_at_page", { path: item.source_path, page: item.page });
        });
      }

      const codeSpan = document.createElement("span");
      codeSpan.className = "browse-item-code";
      codeSpan.textContent = item.code;

      const nameSpan = document.createElement("span");
      nameSpan.className = "browse-item-name";
      nameSpan.textContent = item.name;

      const metaSpan = document.createElement("span");
      metaSpan.className = "browse-item-meta";
      const parts = [item.source_name];
      if (item.page) {
        const isPdf = item.source_path.toLowerCase().endsWith(".pdf");
        if (isPdf) {
          parts.push(`第${item.page}页`);
        } else {
          const loc = item.sheet ? `${item.sheet}-第${item.page}行` : `第${item.page}行`;
          parts.push(loc);
        }
      }
      parts.push(item.source_type === "cnas" ? "CNAS" : "CMA");
      metaSpan.textContent = parts.join(" · ");

      row.appendChild(codeSpan);
      row.appendChild(nameSpan);
      row.appendChild(metaSpan);
      details.appendChild(row);
    }

    browseList.appendChild(details);
  }
}

function openBrowse() {
  browseModal.classList.remove("hidden");
  browseSearch.value = "";
  browseList.textContent = "";
  const loading = document.createElement("div");
  loading.className = "browse-empty";
  loading.textContent = "加载中...";
  browseList.appendChild(loading);

  invoke("get_all_standards")
    .then((entries) => {
      allStandards = entries;
      renderBrowseList("");
    })
    .catch((e) => {
      browseList.textContent = "";
      const err = document.createElement("div");
      err.className = "browse-empty";
      err.textContent = `加载失败：${e}`;
      browseList.appendChild(err);
    });
}

function closeBrowse() {
  browseModal.classList.add("hidden");
}

$("btnBrowse").addEventListener("click", openBrowse);
$("btnCloseBrowse").addEventListener("click", closeBrowse);
browseModal.addEventListener("click", (e) => {
  if (e.target === browseModal) closeBrowse();
});
browseSearch.addEventListener("input", () => {
  renderBrowseList(browseSearch.value.trim());
});
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && !browseModal.classList.contains("hidden")) closeBrowse();
});

// ===== Batch query =====
let batchInputs = [];
let batchFileName = "";
const batchModal = $("batchModal");

function renderBatchFile() {
  const listEl = $("batchFileList");
  listEl.textContent = "";
  if (!batchFileName || batchInputs.length === 0) {
    const empty = document.createElement("span");
    empty.className = "file-list-empty";
    empty.textContent = "未加载";
    listEl.appendChild(empty);
    $("btnBatchView").classList.add("hidden");
    $("btnBatchQuery").disabled = true;
    return;
  }
  const chip = document.createElement("span");
  chip.className = "file-chip";
  chip.title = batchFileName;

  const label = document.createElement("span");
  label.className = "file-chip-name";
  label.textContent = `${batchFileName} (${batchInputs.length})`;

  const rm = document.createElement("button");
  rm.className = "file-chip-remove";
  rm.textContent = "×";
  rm.title = "移除";
  rm.addEventListener("click", async () => {
    await invoke("clear_batch_inputs");
    batchInputs = [];
    batchFileName = "";
    renderBatchFile();
  });

  chip.appendChild(label);
  chip.appendChild(rm);
  listEl.appendChild(chip);
  $("btnBatchView").classList.remove("hidden");
  $("btnBatchQuery").disabled = false;
}

async function loadBatchFile() {
  const path = await open({
    multiple: false,
    filters: [{ name: "Excel", extensions: ["xlsx", "xls"] }],
  });
  if (!path) return;
  try {
    batchInputs = await invoke("parse_batch_file", { path });
    batchFileName = path.split(/[\\/]/).pop();
    renderBatchFile();
  } catch (e) {
    alert(`解析批量文件失败：${e}`);
  }
}

function renderBatchList(filter) {
  const listEl = $("batchList");
  listEl.textContent = "";
  const q = (filter || "").toLowerCase();
  const filtered = q
    ? batchInputs.filter(
        (s) => s.code.toLowerCase().includes(q) || s.name.toLowerCase().includes(q)
      )
    : batchInputs;

  if (filtered.length === 0) {
    const empty = document.createElement("div");
    empty.className = "browse-empty";
    empty.textContent = q ? "无匹配结果" : "暂无已解析的标准";
    listEl.appendChild(empty);
    return;
  }

  for (const item of filtered) {
    const row = document.createElement("div");
    row.className = "browse-item";

    const codeSpan = document.createElement("span");
    codeSpan.className = "browse-item-code";
    codeSpan.textContent = item.code;

    const nameSpan = document.createElement("span");
    nameSpan.className = "browse-item-name";
    nameSpan.textContent = item.name;

    row.appendChild(codeSpan);
    row.appendChild(nameSpan);
    listEl.appendChild(row);
  }
}

function openBatchModal() {
  batchModal.classList.remove("hidden");
  $("batchSearch").value = "";
  renderBatchList("");
}

function closeBatchModal() {
  batchModal.classList.add("hidden");
}

function appendBatchItem(r) {
  const divider = document.createElement("div");
  divider.className = "batch-divider";
  divider.textContent = r.code;
  resultsBody.appendChild(divider);

  const c = createCells();
  renderValidityTo(c.validity, r.validity);
  renderMatch(c.cmaApi, r.cma_api);
  renderMatch(c.cnas, r.cnas);
  renderMatch(c.cmaFile, r.cma_file);
  resultsScroll.scrollTop = resultsScroll.scrollHeight;
}

function setBatchPausedUI(paused) {
  $("btnBatchPause").classList.toggle("hidden", paused);
  $("btnBatchResume").classList.toggle("hidden", !paused);
}

async function runBatch() {
  if (batchInputs.length === 0) return;

  const outPath = await save({
    defaultPath: "批量查询结果.xlsx",
    filters: [{ name: "Excel", extensions: ["xlsx"] }],
  });
  if (!outPath) return;

  const progress = $("batchProgress");
  const fill = $("batchProgressFill");
  const text = $("batchProgressText");
  const status = $("batchStatusText");
  progress.classList.remove("hidden");
  fill.style.width = "0%";
  text.textContent = "0%";
  status.textContent = "准备中...";
  setBatchPausedUI(false);
  $("btnBatchQuery").disabled = true;
  $("btnBatchFile").disabled = true;

  clearResultsArea();

  const unlisten = await listen("batch-progress", (event) => {
    const p = event.payload;
    const pct = Math.round(p.percent);
    fill.style.width = pct + "%";
    text.textContent = pct + "%";
    if (p.warning) {
      status.textContent = p.warning;
      setBatchPausedUI(true);
    } else if (p.done) {
      status.textContent = `查询完成，已保存：${outPath}`;
    } else if (p.paused) {
      status.textContent = "已暂停";
      setBatchPausedUI(true);
    } else {
      status.textContent = `(${p.current + 1}/${p.total}) ${p.code}`;
      setBatchPausedUI(false);
    }
  });

  const unlistenResult = await listen("batch-item-result", (event) => {
    appendBatchItem(event.payload);
  });

  try {
    await invoke("run_batch_query", {
      samrUrl: config.samr_url,
      cmaUrl: config.cma_url,
      outputPath: outPath,
    });
  } catch (e) {
    status.textContent = `批量查询失败：${e}`;
  } finally {
    unlisten();
    unlistenResult();
    $("btnBatchPause").classList.add("hidden");
    $("btnBatchResume").classList.add("hidden");
    $("btnBatchQuery").disabled = false;
    $("btnBatchFile").disabled = false;
  }
}

$("btnBatchPause").addEventListener("click", async () => {
  await invoke("pause_batch_query");
  setBatchPausedUI(true);
  $("batchStatusText").textContent = "已暂停";
});
$("btnBatchResume").addEventListener("click", async () => {
  await invoke("resume_batch_query");
  setBatchPausedUI(false);
});

$("btnBatchFile").addEventListener("click", loadBatchFile);
$("btnBatchView").addEventListener("click", openBatchModal);
$("btnBatchQuery").addEventListener("click", runBatch);
$("btnCloseBatch").addEventListener("click", closeBatchModal);
batchModal.addEventListener("click", (e) => {
  if (e.target === batchModal) closeBatchModal();
});
$("batchSearch").addEventListener("input", () => {
  renderBatchList($("batchSearch").value.trim());
});
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && !batchModal.classList.contains("hidden")) closeBatchModal();
});

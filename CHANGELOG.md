# 标准综合查询器 - 更新日志（Changelog）

## 项目概况

Tauri 2 + Rust + 原生 JS 桌面应用，用于查询中国标准（GB/T 等）的有效性。

- **仓库**：https://github.com/d3intran/SearchApp.git
- **当前版本**：v0.5.0
- **编译产物路径**：`C:\Code\Rust_Builds\release\标准综合查询器.exe`
- **更新服务**：Cloudflare Worker `update.2005666.xyz`，R2 桶 `app-releases`

## 编译命令

```bash
# 环境：无 npm/node，使用 deno 执行 npm 包
deno run -A npm:@tauri-apps/cli build
```

编译前需确保旧进程已关闭（否则 exe 被锁定）：
```bash
taskkill //F //IM "标准综合查询器.exe"
```

## 更新服务架构

- **Worker 项目**：`E:\GKY\CMA查询\update-worker\`
- **R2 桶**：`app-releases`
  - `searchapp/version.json` — 版本信息（version/url/notes/date）
  - `searchapp/StandardQuery.exe` — 编译产物（R2 key 未改名，仅 Content-Disposition 文件名改了）
- **接口**：
  - `GET /searchapp` → 返回 version.json
  - `GET /searchapp/download` → 返回 exe 文件
- **注意**：请求需带 User-Agent `Searchapp`，否则被 Cloudflare WAF 拦截

### 上传命令

```bash
cd "E:\GKY\CMA查询\update-worker"

# 上传 exe
deno run -A npm:wrangler r2 object put app-releases/searchapp/StandardQuery.exe --file "C:\Code\Rust_Builds\release\标准综合查询器.exe" --remote

# 上传 version.json（先写入临时文件）
printf '{"version":"x.y.z","url":"https://update.2005666.xyz/searchapp/download","notes":"更新说明","date":"yyyy-mm-dd"}' > /tmp/version.json
deno run -A npm:wrangler r2 object put app-releases/searchapp/version.json --file /tmp/version.json --remote

# 部署 Worker（修改了 src/index.ts 后）
deno run -A npm:wrangler deploy
```

## 更新流程（客户端）

1. 请求 `https://update.2005666.xyz/searchapp` 获取 JSON
2. 比较远程 version 与本地 `CARGO_PKG_VERSION`（编译时注入）
3. 远程 > 本地 → 流式下载 exe，通过 Tauri 事件 `update-progress` 推送进度
4. 下载完成 → 按钮变为"重启使用新版本"
5. 用户点击 → 生成 PowerShell 脚本（`update.ps1`，带 UTF-8 BOM）替换 exe 并重启
   - 使用 `powershell.exe -ExecutionPolicy Bypass -File update.ps1` 执行
   - PowerShell 原生支持 UTF-8 路径，避免 cmd 的中文乱码问题

## 关键文件

| 文件 | 作用 |
|------|------|
| `src-tauri/src/updater.rs` | 更新逻辑（check/download/apply），使用 PowerShell 脚本替换 exe |
| `src-tauri/src/batch.rs` | 批量查询模块（解析文件、串行查询四源、导出 Excel） |
| `src-tauri/src/parsers/pdf_parser.rs` | PDF 解析（含页码追踪） |
| `src-tauri/src/parsers/excel_parser.rs` | Excel 解析 |
| `src-tauri/src/services/local_matcher.rs` | 本地标准匹配 + 浏览数据 |
| `src-tauri/src/commands.rs` | 所有 Tauri 命令 |
| `src-tauri/examples/` | 终端验证工具（批量提取/批量查询） |
| `src/main.js` | 前端全部逻辑 |
| `index.html` | 单页 HTML |
| `src/styles.css` | 全部样式 |

## 注意事项

1. **环境**：系统无 npm/node，所有 JS 工具通过 `deno run -A npm:xxx` 执行
2. **exe 名称**：通过 Cargo.toml `[[bin]] name = "标准综合查询器"` 控制，未来编译始终为此名
3. **版本号**：需同时改 `Cargo.toml` version 和 `tauri.conf.json` version
4. **R2 key**：exe 的 R2 key 仍为 `searchapp/StandardQuery.exe`（未改），URL 路径 `/searchapp/download` 不变
5. **WAF**：curl 直接请求会被拦截，需加 `-A "Searchapp"`
6. **编译锁定**：编译前必须关闭正在运行的 exe，否则报"拒绝访问"
7. **PDF 打开**：使用 `cmd /c start "" "file:///path#page=N"` 在浏览器中定位页码
8. **状态持久化**：已加载文件路径保存在 exe 同目录 `state.json`，重启自动恢复
9. **Excel 解析**：支持同行跨单元格取名、《》括号提取、去重保留有名称+行号小的条目

## v0.2.0 功能清单

- 标准浏览面板（按前缀分组、编号排序、实时搜索、点击打开 PDF 定位页码）
- 更新流程优化（进度条 + 手动确认重启）
- 设置面板按钮左右对称
- 已选文件列表在各自按钮正下方
- 应用更名为"标准综合查询器"
- Excel 解析增强：
  - 标准号+名称同格提取（《》括号、代码后文本、代码前文本）
  - 同行跨单元格自动查找标准名（标准号和名称分开存放时）
  - 同文件去重：优先保留有名称的，其次保留行号更小的
  - 位置定位显示子表名+行号（如"能力表标准-第112行"）
  - 自动去除【是/否】噪音，遇下一个标准号截断
- 状态持久化：重启后自动恢复已加载的附表文件

## v0.3.x 变更记录（2026-08-03）

### v0.3.0：批量查询 + 浏览器快捷查询
- **批量查询功能**：导入 Excel 提取标准号，串行查询 SAMR/CNAS/CMA附表/CMA库 四源并导出结果 Excel
- **进度条**：批量查询时实时显示进度，逐项展示当前查询标准号
- **结果导出**：查询完成后弹出保存对话框选择位置，按 标准号/标准名/四源结果 六列导出，带列宽+自动换行格式
- **标准名智能提取**：优先从查询结果中提取标准名，四源皆空时回退到输入表格中的名称
- **防限速**：在线查询间随机延迟 2-5 秒
- **地球图标**：有效性查询结果右侧新增地球图标按钮，一键在浏览器打开国标平台查询（URL 拼接 `std.samr.gov.cn/search/std?q=...`）
- **新文件**：`src-tauri/src/batch.rs`（批量查询模块）、`src-tauri/examples/batch_query.rs`（终端验证工具）

### v0.3.1 ~ v0.3.3：修复重启更新中文路径问题
- **问题**：exe 在中文路径下（如 `E:\新建文件夹\标准综合查询器\`），重启更新时 cmd 报"找不到文件"错误
- **v0.3.1**：尝试在 bat 开头加 `chcp 65001` 切换 UTF-8 编码 → 仍不稳定
- **v0.3.2**：改用 PowerShell 脚本 `update.ps1` 替代 `update.bat` → PowerShell 5.x 默认用系统 ANSI 编码读取，依然乱码
- **v0.3.3**：给 `.ps1` 文件添加 UTF-8 BOM（`EF BB BF`）→ Windows PowerShell 正确识别 UTF-8，彻底修复
- **经验**：Windows 上中文路径 + 脚本文件必须注意编码：
  - cmd `.bat`：`chcp 65001` 不可靠
  - PowerShell `.ps1`：必须带 UTF-8 BOM

### v0.3.4 ~ v0.3.5：批量查询实时渲染、暂停恢复与增量保存
- **v0.3.4**：批量查询时每项结果实时渲染到查询面板，提升可视化反馈
- **v0.3.5**：
  - **增量保存与文件占用处理**：批量查询前预选保存路径，查询过程中实时增量写入 Excel 文件；若目标 Excel 被 Excel 等软件占用打开，自动暂停查询并弹框提示释放占用，点击「恢复」后接着写入
  - **暂停 / 恢复控制**：支持中途手动或因异常暂停批量查询
  - **查询面板布局调整**：统一多条批量查询结果在面板中的网格与滚动展现

### v0.3.6：PDF条款前缀自动清洗与更新同步
- **标准名称清洗**：自动剔除 PDF 解析标准名称前缀的章节条款号（如 `4.1`、`7.5.9`、`8.1` 等），确保标准名称干净规范
- **网页快捷跳转**：支持单条查询结果处点击 CMA 🌐 图标跳转平台并自动复制当前标准号至系统剪贴板

### v0.3.7：标准名查询 + 斜杠归一化 + 结果区重构
- **四源标准名查询**：输入不含标准号格式（字母+数字）时自动走名称模式
  - SAMR：按名称搜索并列出相关标准卡片（标准号+完整名称+状态）
  - CMA 能力库：改用 `standardMethod` 参数查询
  - CNAS/CMA 本地附表：对条目名称做包含匹配（忽略空格/大小写），最多 10 条
- **斜杠归一化**：`normalize()` 去除 `/`，`GYT222` 与 `GY/T222` 等价（SAMR 比对、本地索引、CMA 比对全部生效）
- **CMA 库斜杠变体重试**：接口不做斜杠归一化，首次无结果且前缀为三个连续字母时自动在第二字母后补 `/` 重试（GYT222 → GY/T222）
- **SAMR 名称解析修复**：按名称搜索时 SAMR 用 `<sacinfo>` 标签高亮搜索词，原正则在首个 `<` 处截断导致名称为空；改为捕获到 `</a>` 后统一剥标签
- **分页提示**：
  - SAMR 解析页面 `totalPages:N`，超过 1 页提示"共 N 页结果，当前仅显示第 1 页"
  - CMA 库利用接口 `total` 字段，总数超过当页条数时提示去网站查看
- **CMA 库显示全部结果**：取消原来只显示 5 条的限制，展示当页全部结果（最多 20 条）
- **结果区重构**：四个结果列合并为单卡片统一滚动区，批量查询每个标准一行四格（高度自动对齐），列顺序为 有效性 | CMA能力项目库 | CNAS附表 | CMA附表，列标题间淡虚线分隔
- **UI 细节**：
  - 输入框标签改为"标准号支持不带年份查询"，下方加示例两行（GYT222-2023 / GY/T222 / GYT222 / 数字电视转播车技术要求和测量方法）
  - 结果文字可选中复制（`user-select: text`）
  - "查看已解析标准"改为书本图标按钮，位于批量文件芯片之后，未加载时隐藏

### v0.3.9：输入区文案微调
- 移除输入框占位文字（"例如：GB/T 28435-2012"）
- 示例第二行缩进两个全角空格，与第一行标准号对齐

### v0.5.0：移除本地附表现行判定回退
- **标准有效性完全以 SAMR 在线查询结果为准**：删除"SAMR 未收录时依据本地附表判定为现行"的回退逻辑
- 删除 `is_in_local_files` 方法及相关代码
- `GD/J` 等 SAMR 未收录标准的有效性列显示"无匹配结果"，本地附表查询列仍正常显示收录情况
- README 对应说明同步更新

### 新增依赖
- `rust_xlsxwriter` 0.97 — Excel 导出（带格式、列宽、自动换行）
- `rand` 0.10 — 随机延迟（`RngExt::random_range`，注意 ThreadRng 非 Send，需在 await 前丢弃）

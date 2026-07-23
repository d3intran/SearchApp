# 标准综合查询器

基于 **Tauri 2 + Rust + 原生 HTML/CSS/JS** 的绿色版桌面工具。输入标准号即可一键查询其在多个数据源中的匹配情况和有效性状态。

单文件免安装，双击即用。

## 功能

输入一个标准号（如 `GB/T 28435-2012` 或 `GY/T 222-2006`），同时查询：

| 数据源 | 方式 | 查什么 |
|--------|------|--------|
| 全国标准信息公共服务平台 (std.samr.gov.cn) | 在线 HTML 解析 | 标准是否现行/废止、替代关系 |
| CMA 能力项目库 (cma.caqit.org.cn) | 在线 JSON API | 标准是否在能力项目库中、备注信息 |
| CNAS 附表 | 本地 PDF/Excel | 标准是否在 CNAS 认可附表中 |
| CMA 附表 | 本地 PDF/Excel | 标准是否在 CMA 资质附表中 |

当 SAMR 未收录某标准（如 `GD/J` 广电行业标准）时，若该标准存在于已加载的本地附表中，则判定为"现行（依据本地附表判定）"。

### 标准浏览面板

点击标题栏书本图标，可查看所有已从 PDF/Excel 解析出的标准：

- 按标准号前缀分组（GB、GB/T、JJF 等），可折叠
- 组内按编号数值从小到大排序
- 实时搜索过滤（标准号或名称）
- 点击 PDF 来源的标准，在浏览器中打开 PDF 并定位到对应页码

### 软件更新

设置面板 → 检查软件更新：

- 自动检测新版本（Cloudflare Worker + R2）
- 下载时显示实时进度条
- 下载完成后按钮变为"重启使用新版本"，用户确认后替换并重启

### 结果配色

- 完全匹配 / 现行 → 绿色
- 未完全匹配 → 黄色
- 废止 / 无匹配 → 红色

## 使用方式

1. 双击 `标准综合查询器.exe` 运行（无需安装任何运行时）
2. 选择 CNAS/CMA 附表文件（PDF 或 Excel，支持多文件）
3. 输入标准号，回车或点击查询
4. 查看有效性结果 + 三栏匹配结果

## 项目结构

```
├── index.html              # 前端入口（单页）
├── package.json            # 前端依赖（Tauri API + Vite）
├── vite.config.ts
├── src/
│   ├── main.js             # 前端逻辑
│   └── styles.css          # Win11 Fluent 风格样式
└── src-tauri/              # Rust 后端
    ├── Cargo.toml
    ├── tauri.conf.json
    └── src/
        ├── lib.rs / main.rs
        ├── commands.rs             # Tauri command 层
        ├── updater.rs              # 软件更新（检查/下载/应用）
        ├── config.rs               # 配置持久化
        ├── services/
        │   ├── standard_parser.rs  # 标准号提取与归一化
        │   ├── cma_api.rs          # CMA 能力项目库在线查询
        │   ├── samr_status.rs      # 国标有效性查询（HTML 解析）
        │   └── local_matcher.rs    # 本地附表匹配 + 浏览数据
        └── parsers/
            ├── mod.rs              # 文件路由
            ├── excel_parser.rs     # calamine 解析 Excel
            └── pdf_parser.rs       # pdf-rs 解析 PDF（含页码追踪）
```

## 技术栈

| 项目 | 选择 |
|------|------|
| 框架 | Tauri 2 |
| 后端 | Rust |
| 前端 | 原生 HTML/CSS/JS + Vite |
| HTTP | reqwest（stream 下载进度） |
| PDF 解析 | pdf 0.9（底层内容流解析） |
| Excel 解析 | calamine 0.26 |
| 正则 | regex |
| 更新服务 | Cloudflare Worker + R2 |

## 构建

环境要求：Rust 工具链、Deno（无需 Node.js）。

```bash
# 安装前端依赖
deno install

# 开发模式
deno run -A npm:@tauri-apps/cli dev

# 发布构建
deno run -A npm:@tauri-apps/cli build
```

产物为 `标准综合查询器.exe`，单文件绿色免安装。

## 查询逻辑要点

- **标准号归一化**：小写、去空格、中文标点转英文（`－`→`-`，`：`→`:`）
- **CMA API**：查询参数必须去空格（`GY/T198-2003`），需携带 `Referer` 头
- **SAMR HTML**：按卡片起始标签分割，逐段提取标准号/名称/状态
- **替代关系**：从同页搜索结果中找同基础编号且状态为"现行"的条目
- **模糊匹配**：输入无年份时做前缀匹配，列出所有相关版本
- **PDF 表格检测**：通过线段聚类识别表格网格，按列拼接文本修复跨行标准号

## 已知限制

- `GD/J` 等广电行业标准不在 SAMR 平台，需通过本地附表补充判定
- `GY 5xxx` 工程建设标准 SAMR 和本地附表均可能未收录
- SAMR 页面改版时正则需同步更新
- PDF 页码定位依赖浏览器 PDF 查看器的 `#page=N` 支持

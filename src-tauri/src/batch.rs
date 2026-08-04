use crate::parsers::excel_parser;
use crate::services::{cma_api, samr_status, standard_parser};
use crate::AppState;
use rand::RngExt;
use regex::Regex;
use rust_xlsxwriter::{Format, FormatAlign, Workbook};
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize, Clone)]
pub struct BatchInput {
    pub code: String,
    pub name: String,
}

#[derive(Clone)]
pub struct BatchRow {
    pub code: String,
    pub name: String,
    pub validity: String,
    pub cnas: String,
    pub cma_file: String,
    pub cma_api: String,
}

#[derive(Serialize, Clone)]
pub struct BatchProgress {
    pub current: usize,
    pub total: usize,
    pub code: String,
    pub percent: f64,
    pub done: bool,
    pub paused: bool,
    pub warning: String,
}

#[derive(Serialize, Clone)]
pub struct BatchItemResult {
    pub code: String,
    pub validity: samr_status::ValidityResult,
    pub cnas: crate::services::local_matcher::MatchResult,
    pub cma_file: crate::services::local_matcher::MatchResult,
    pub cma_api: cma_api::QueryResult,
}

pub struct BatchControl {
    pub paused: bool,
}

#[tauri::command]
pub fn pause_batch_query(state: State<'_, AppState>) {
    state.batch_control.lock().unwrap().paused = true;
}

#[tauri::command]
pub fn resume_batch_query(state: State<'_, AppState>) {
    state.batch_control.lock().unwrap().paused = false;
}

fn set_paused(app: &AppHandle, paused: bool) {
    let state = app.state::<AppState>();
    state.batch_control.lock().unwrap().paused = paused;
}

fn is_paused(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let guard = state.batch_control.lock().unwrap();
    let paused = guard.paused;
    drop(guard);
    paused
}

async fn wait_while_paused(app: &AppHandle) {
    while is_paused(app) {
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

#[tauri::command]
pub fn parse_batch_file(path: String, state: State<'_, AppState>) -> Result<Vec<BatchInput>, String> {
    let entries = excel_parser::parse(&path)?;
    let mut inputs: Vec<BatchInput> = entries
        .into_iter()
        .map(|e| BatchInput { code: e.code, name: e.name })
        .collect();
    inputs.sort_by(|a, b| a.code.cmp(&b.code));
    inputs.dedup_by(|a, b| a.code == b.code);
    *state.batch_inputs.lock().unwrap() = inputs.clone();
    Ok(inputs)
}

#[tauri::command]
pub fn get_batch_inputs(state: State<'_, AppState>) -> Vec<BatchInput> {
    state.batch_inputs.lock().unwrap().clone()
}

#[tauri::command]
pub fn clear_batch_inputs(state: State<'_, AppState>) {
    state.batch_inputs.lock().unwrap().clear();
}

async fn random_delay() {
    let ms = {
        let mut rng = rand::rng();
        rng.random_range(2000..5000)
    };
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

fn extract_name_from_text(text: &str, code: &str) -> String {
    let std_re = Regex::new(
        r"([A-Za-z]+[/]?[A-Za-z]*)\s*([0-9]+(?:[.\-][0-9]+)*)\s*[-\u{FF0D}\u{2014}]\s*([0-9]{4})",
    )
    .unwrap();
    let target = standard_parser::normalize(code);

    for line in text.lines() {
        let line = line.trim().trim_start_matches('·').trim();
        for cap in std_re.captures_iter(line) {
            let matched_code = format!("{} {}-{}", cap[1].replace(' ', ""), &cap[2], &cap[3]);
            if standard_parser::normalize(&matched_code) != target {
                continue;
            }
            let end = cap.get(0).unwrap().end();
            let rest = &line[end..];
            let name = Regex::new(r"[（(][^）)]*[）)]\s*$")
                .unwrap()
                .replace_all(rest.trim(), "")
                .trim()
                .trim_matches(|c: char| c == '、' || c == '，' || c == ',' || c == ' ' || c == ':')
                .trim()
                .to_string();
            if name.chars().count() >= 2 {
                return name;
            }
        }
    }
    String::new()
}

fn write_output(rows: &[BatchRow], path: &str) -> Result<(), String> {
    let header_fmt = Format::new()
        .set_bold()
        .set_text_wrap()
        .set_align(FormatAlign::VerticalCenter);
    let cell_fmt = Format::new()
        .set_text_wrap()
        .set_align(FormatAlign::Top);

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();

    let widths = [18.0, 42.0, 46.0, 46.0, 42.0, 42.0];
    for (c, w) in widths.iter().enumerate() {
        ws.set_column_width(c as u16, *w).map_err(|e| e.to_string())?;
    }

    let headers = ["标准号", "标准名", "有效性(SAMR)", "CMA能力项目库", "CNAS附表", "CMA附表"];
    for (c, h) in headers.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &header_fmt)
            .map_err(|e| e.to_string())?;
    }
    for (r, row) in rows.iter().enumerate() {
        let vals = [&row.code, &row.name, &row.validity, &row.cma_api, &row.cnas, &row.cma_file];
        for (c, val) in vals.iter().enumerate() {
            ws.write_string_with_format((r + 1) as u32, c as u16, (*val).clone(), &cell_fmt)
                .map_err(|e| e.to_string())?;
        }
    }
    wb.save(path).map_err(|e| format!("保存Excel失败：{}", e))
}

#[tauri::command]
pub async fn run_batch_query(
    app: AppHandle,
    samr_url: String,
    cma_url: String,
    output_path: String,
) -> Result<usize, String> {
    let inputs = app.state::<AppState>().batch_inputs.lock().unwrap().clone();
    if inputs.is_empty() {
        return Err("没有可查询的标准，请先选择批量文件".into());
    }

    set_paused(&app, false);
    let total = inputs.len();
    let mut rows: Vec<BatchRow> = Vec::new();

    // Write header first to fail fast if the file is locked (e.g. opened in Excel)
    write_output(&rows, &output_path).map_err(|_| {
        "无法写入目标文件，请确认该 Excel 文件已关闭后重试".to_string()
    })?;

    for (i, input) in inputs.iter().enumerate() {
        wait_while_paused(&app).await;

        let code = standard_parser::extract_code(&input.code);

        let _ = app.emit(
            "batch-progress",
            BatchProgress {
                current: i,
                total,
                code: code.clone(),
                percent: (i as f64 / total as f64) * 100.0,
                done: false,
                paused: false,
                warning: String::new(),
            },
        );

        let validity = samr_status::query(&code, &samr_url).await;
        let validity_text = validity
            .lines
            .iter()
            .map(|l| l.text.clone())
            .collect::<Vec<_>>()
            .join("\n");

        random_delay().await;

        let cnas_result = {
            let state = app.state::<AppState>();
            let matcher = state.matcher.lock().unwrap();
            matcher.query_cnas(&code)
        };
        let cma_file_result = {
            let state = app.state::<AppState>();
            let matcher = state.matcher.lock().unwrap();
            matcher.query_cma(&code)
        };

        let cma_api_result = cma_api::query(&code, &cma_url).await;

        let _ = app.emit(
            "batch-item-result",
            BatchItemResult {
                code: code.clone(),
                validity: validity.clone(),
                cnas: cnas_result.clone(),
                cma_file: cma_file_result.clone(),
                cma_api: cma_api_result.clone(),
            },
        );

        let cnas_text = cnas_result.message.clone();
        let cma_file_text = cma_file_result.message.clone();
        let cma_api_text = cma_api_result.message.clone();

        let mut name = String::new();
        for text in [&validity_text, &cnas_text, &cma_file_text, &cma_api_text] {
            let n = extract_name_from_text(text, &code);
            if !n.is_empty() {
                name = n;
                break;
            }
        }
        let name = if name.is_empty() { input.name.clone() } else { name };

        rows.push(BatchRow {
            code,
            name,
            validity: validity_text,
            cnas: cnas_text,
            cma_file: cma_file_text,
            cma_api: cma_api_text,
        });

        // Incremental save; if the file is locked, pause and wait for user to resume
        loop {
            match write_output(&rows, &output_path) {
                Ok(_) => break,
                Err(_) => {
                    set_paused(&app, true);
                    let _ = app.emit(
                        "batch-progress",
                        BatchProgress {
                            current: i + 1,
                            total,
                            code: String::new(),
                            percent: ((i + 1) as f64 / total as f64) * 100.0,
                            done: false,
                            paused: true,
                            warning: "结果文件写入失败（可能正被 Excel 打开），请关闭该文件后点击「恢复」".into(),
                        },
                    );
                    wait_while_paused(&app).await;
                }
            }
        }

        if i < total - 1 {
            random_delay().await;
        }
    }

    let _ = app.emit(
        "batch-progress",
        BatchProgress {
            current: total,
            total,
            code: String::new(),
            percent: 100.0,
            done: true,
            paused: false,
            warning: String::new(),
        },
    );

    Ok(total)
}

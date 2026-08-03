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

    let headers = ["标准号", "标准名", "有效性(SAMR)", "CNAS附表", "CMA附表", "CMA能力项目库"];
    for (c, h) in headers.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &header_fmt)
            .map_err(|e| e.to_string())?;
    }
    for (r, row) in rows.iter().enumerate() {
        let vals = [&row.code, &row.name, &row.validity, &row.cnas, &row.cma_file, &row.cma_api];
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
) -> Result<usize, String> {
    let inputs = app.state::<AppState>().batch_inputs.lock().unwrap().clone();
    if inputs.is_empty() {
        return Err("没有可查询的标准，请先选择批量文件".into());
    }
    let total = inputs.len();
    let mut rows = Vec::new();

    for (i, input) in inputs.iter().enumerate() {
        let code = standard_parser::extract_code(&input.code);

        let _ = app.emit(
            "batch-progress",
            BatchProgress {
                current: i,
                total,
                code: code.clone(),
                percent: (i as f64 / total as f64) * 100.0,
                done: false,
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

        let cnas_text = {
            let state = app.state::<AppState>();
            let matcher = state.matcher.lock().unwrap();
            matcher.query_cnas(&code).message
        };
        let cma_file_text = {
            let state = app.state::<AppState>();
            let matcher = state.matcher.lock().unwrap();
            matcher.query_cma(&code).message
        };

        let cma_api_result = cma_api::query(&code, &cma_url).await;
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
        },
    );

    let state = app.state::<AppState>();
    *state.batch_results.lock().unwrap() = rows;
    Ok(total)
}

#[tauri::command]
pub fn save_batch_result(output_path: String, state: State<'_, AppState>) -> Result<String, String> {
    let rows = state.batch_results.lock().unwrap().clone();
    if rows.is_empty() {
        return Err("没有可保存的批量查询结果".into());
    }
    write_output(&rows, &output_path)?;
    Ok(output_path)
}

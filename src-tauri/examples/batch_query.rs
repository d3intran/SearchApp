use calamine::{open_workbook_auto, Reader};
use rand::RngExt;
use regex::Regex;
use rust_xlsxwriter::Workbook;
use standard_query::services::{cma_api, local_matcher::LocalFileMatcher, samr_status, standard_parser};
use std::time::Duration;

const SAMR_URL: &str = "https://std.samr.gov.cn";
const CMA_URL: &str = "https://cma.caqit.org.cn";

const CNAS_FILES: &[&str] = &[
    r"E:\GKY\文档\2026品种表查询标准.xlsx",
    r"E:\GKY\文档\2026品种表查询标准备份.xlsx",
    r"E:\GKY\文档\标准有效性确认表格7-16-2.xlsx",
];

const CMA_FILES: &[&str] = &[
    r"E:\GKY\CMA查询\2024能力附表.pdf",
    r"E:\GKY\CMA查询\广播电视设备器材入网认定品种表（2026版）.pdf",
];

const INPUT_XLSX: &str = r"E:\GKY\Tauri\批量测试输入.xlsx";
const OUTPUT_XLSX: &str = r"E:\GKY\Tauri\批量查询结果.xlsx";

fn load_input() -> Vec<(String, String)> {
    let mut wb = open_workbook_auto(INPUT_XLSX).expect("打开输入Excel失败");
    let sheet = wb.sheet_names()[0].clone();
    let range = wb.worksheet_range(&sheet).expect("读取工作表失败");
    range
        .rows()
        .skip(1)
        .filter_map(|row| {
            let code = row.first()?.to_string().trim().to_string();
            if code.is_empty() {
                return None;
            }
            let name = row.get(1).map(|c| c.to_string().trim().to_string()).unwrap_or_default();
            Some((code, name))
        })
        .collect()
}

/// 从查询结果文本中提取与目标标准号对应的标准名称
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

async fn random_delay() {
    let mut rng = rand::rng();
    let ms = rng.random_range(2000..5000);
    println!("    （随机延迟 {} ms）", ms);
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

#[tokio::main]
async fn main() {
    if std::env::args().nth(1).as_deref() == Some("--reformat") {
        let rows = read_existing(OUTPUT_XLSX);
        write_output(&rows);
        println!("已重新格式化：{}", OUTPUT_XLSX);
        return;
    }

    println!("加载 CNAS 附表...");
    let mut matcher = LocalFileMatcher::new();
    for p in CNAS_FILES {
        match matcher.add_cnas(p) {
            Ok(_) => println!("  已加载：{}", p),
            Err(e) => println!("  加载失败：{}（{}）", p, e),
        }
    }
    println!("加载 CMA 附表...");
    for p in CMA_FILES {
        match matcher.add_cma(p) {
            Ok(_) => println!("  已加载：{}", p),
            Err(e) => println!("  加载失败：{}（{}）", p, e),
        }
    }

    let input = load_input();
    println!("\n共 {} 个标准号，开始串行查询...\n", input.len());

    let mut rows: Vec<(String, String, String, String, String, String)> = Vec::new();

    for (i, (raw_code, input_name)) in input.iter().enumerate() {
        let code = standard_parser::extract_code(raw_code);
        println!("[{}/{}] {}", i + 1, input.len(), code);

        let validity = samr_status::query(&code, SAMR_URL).await;
        let validity_text = validity
            .lines
            .iter()
            .map(|l| l.text.clone())
            .collect::<Vec<_>>()
            .join("\n");
        println!("  SAMR: {}", validity.lines.first().map(|l| l.text.as_str()).unwrap_or(""));

        random_delay().await;

        let cnas = matcher.query_cnas(&code);
        let cnas_text = cnas.message.clone();
        println!("  CNAS附表: {}", cnas.message.lines().next().unwrap_or(""));

        let cma_file = matcher.query_cma(&code);
        let cma_file_text = cma_file.message.clone();
        println!("  CMA附表: {}", cma_file.message.lines().next().unwrap_or(""));

        let cma_api_result = cma_api::query(&code, CMA_URL).await;
        let cma_api_text = cma_api_result.message.clone();
        println!("  CMA库: {}", cma_api_result.message.lines().next().unwrap_or(""));

        // 标准名优先取查询结果中的，四个源都没有才回退到输入表格的名称
        let mut name = String::new();
        for text in [&validity_text, &cnas_text, &cma_file_text, &cma_api_text] {
            let n = extract_name_from_text(text, &code);
            if !n.is_empty() {
                name = n;
                break;
            }
        }
        let name = if name.is_empty() { input_name.clone() } else { name };

        rows.push((code, name, validity_text, cnas_text, cma_file_text, cma_api_text));

        if i < input.len() - 1 {
            random_delay().await;
        }
    }

    write_output(&rows);
    println!("\n查询完成，结果已保存到：{}", OUTPUT_XLSX);
}

fn read_existing(path: &str) -> Vec<(String, String, String, String, String, String)> {
    let mut wb = open_workbook_auto(path).expect("打开结果Excel失败");
    let sheet = wb.sheet_names()[0].clone();
    let range = wb.worksheet_range(&sheet).expect("读取工作表失败");
    range
        .rows()
        .skip(1)
        .map(|row| {
            let get = |i: usize| {
                row.get(i)
                    .map(|c| c.to_string().replace("_x000D_", "\n").replace("_x000A_", "\n"))
                    .unwrap_or_default()
            };
            (get(0), get(1), get(2), get(3), get(4), get(5))
        })
        .collect()
}

fn write_output(rows: &[(String, String, String, String, String, String)]) {
    use rust_xlsxwriter::{Format, FormatAlign};

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
        ws.set_column_width(c as u16, *w).unwrap();
    }

    let headers = ["标准号", "标准名", "有效性(SAMR)", "CNAS附表", "CMA附表", "CMA能力项目库"];
    for (c, h) in headers.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &header_fmt).unwrap();
    }
    for (r, row) in rows.iter().enumerate() {
        for (c, val) in [&row.0, &row.1, &row.2, &row.3, &row.4, &row.5].iter().enumerate() {
            ws.write_string_with_format((r + 1) as u32, c as u16, (*val).clone(), &cell_fmt)
                .unwrap();
        }
    }
    wb.save(OUTPUT_XLSX).expect("保存Excel失败");
}

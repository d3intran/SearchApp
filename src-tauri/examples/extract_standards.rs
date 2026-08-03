use rust_xlsxwriter::{Workbook, XlsxError};
use standard_query::parsers::excel_parser;

fn main() -> Result<(), XlsxError> {
    let src = r"E:\GKY\A-old\2026品种表查询标准备份.xlsx";
    let mut entries = excel_parser::parse(src).expect("解析Excel失败");

    entries.sort_by(|a, b| a.code.cmp(&b.code));
    entries.truncate(20);

    println!("共提取 {} 个标准号：\n", entries.len());
    println!("{:<3} {:<24} {}", "序号", "标准号", "标准名称");
    println!("{}", "-".repeat(70));
    for (i, e) in entries.iter().enumerate() {
        println!("{:<4} {:<24} {}", i + 1, e.code, e.name);
    }

    let out = r"E:\GKY\Tauri\批量测试输入.xlsx";
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "标准号")?;
    ws.write_string(0, 1, "标准名称")?;
    for (i, e) in entries.iter().enumerate() {
        ws.write_string((i + 1) as u32, 0, &e.code)?;
        ws.write_string((i + 1) as u32, 1, &e.name)?;
    }
    wb.save(out)?;
    println!("\n已保存到：{}", out);

    Ok(())
}

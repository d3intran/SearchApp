use pdf::content::{Op, Matrix, TextDrawAdjusted};
use pdf::file::FileOptions;
use pdf::font::ToUnicodeMap;
use pdf::object::Resolve;
use regex::Regex;
use std::collections::HashMap;

use crate::services::local_matcher::StandardEntry;
use crate::services::standard_parser;

// ---------- geometry helpers ----------

fn mat_mul(a: &Matrix, b: &Matrix) -> Matrix {
    Matrix {
        a: a.a * b.a + a.b * b.c,
        b: a.a * b.b + a.b * b.d,
        c: a.c * b.a + a.d * b.c,
        d: a.c * b.b + a.d * b.d,
        e: a.e * b.a + a.f * b.c + b.e,
        f: a.e * b.b + a.f * b.d + b.f,
    }
}

fn transform(m: &Matrix, x: f32, y: f32) -> (f32, f32) {
    (m.a * x + m.c * y + m.e, m.b * x + m.d * y + m.f)
}

fn translate(tx: f32, ty: f32) -> Matrix {
    Matrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: tx, f: ty }
}

// ---------- intermediate data ----------

struct Word {
    x0: f32,
    y: f32, // top-left baseline
    text: String,
}

struct Segment {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

struct FontInfo {
    map: Option<ToUnicodeMap>,
    is_cid: bool,
}

// ---------- text decoding ----------

fn decode_bytes(bytes: &[u8], info: Option<&FontInfo>) -> String {
    let mut out = String::new();
    let (map, width) = match info {
        Some(fi) => (fi.map.as_ref(), if fi.is_cid { 2 } else { 1 }),
        None => (None, 2),
    };
    let mut i = 0;
    while i + width <= bytes.len() {
        let code = if width == 2 {
            ((bytes[i] as u16) << 8) | (bytes[i + 1] as u16)
        } else {
            bytes[i] as u16
        };
        if let Some(m) = map {
            if let Some(s) = m.get(code) {
                out.push_str(s);
            }
        }
        i += width;
    }
    out
}

// ---------- page extraction ----------

struct PageData {
    words: Vec<Word>,
    segments: Vec<Segment>,
}

fn extract_page<R: Resolve>(
    page: &pdf::object::Page,
    resolver: &R,
) -> Result<PageData, pdf::error::PdfError> {
    let mut words = Vec::new();
    let mut segments = Vec::new();

    let media = page.media_box()?;
    let (m_left, m_top) = (media.left, media.top);

    // font info map
    let mut fonts: HashMap<String, FontInfo> = HashMap::new();
    if let Ok(resources) = page.resources() {
        for (name, font_ref) in resources.fonts.iter() {
            if let Ok(font) = font_ref.load(resolver) {
                let map = font.to_unicode(resolver).and_then(|r| r.ok());
                let is_cid = font.is_cid();
                fonts.insert(name.as_str().to_string(), FontInfo { map, is_cid });
            }
        }
    }

    let content = match &page.contents {
        Some(c) => c,
        None => return Ok(PageData { words, segments }),
    };
    let ops = content.operations(resolver)?;

    let mut ctm = Matrix::default();
    let mut ctm_stack: Vec<Matrix> = Vec::new();
    let mut text_matrix = Matrix::default();
    let mut line_matrix = Matrix::default();
    let mut leading: f32 = 0.0;
    let mut cur_font: Option<String> = None;
    let mut cur_size: f32 = 0.0;

    // path building
    let mut path_pts: Vec<(f32, f32)> = Vec::new();
    let mut sub_start: (f32, f32) = (0.0, 0.0);
    let mut cur_pt: (f32, f32) = (0.0, 0.0);

    let to_page = |ctm: &Matrix, x: f32, y: f32| -> (f32, f32) {
        let (px, py) = transform(ctm, x, y);
        (px - m_left, m_top - py)
    };

    for op in &ops {
        match op {
            Op::Save => {
                ctm_stack.push(ctm);
            }
            Op::Restore => {
                ctm = ctm_stack.pop().unwrap_or_else(Matrix::default);
            }
            Op::Transform { matrix } => {
                ctm = mat_mul(matrix, &ctm);
            }
            // ----- text state -----
            Op::SetTextMatrix { matrix } => {
                text_matrix = *matrix;
                line_matrix = *matrix;
            }
            Op::MoveTextPosition { translation } => {
                let t = translate(translation.x, translation.y);
                line_matrix = mat_mul(&t, &line_matrix);
                text_matrix = line_matrix;
            }
            Op::TextNewline => {
                let t = translate(0.0, -leading);
                line_matrix = mat_mul(&t, &line_matrix);
                text_matrix = line_matrix;
            }
            Op::Leading { leading: l } => leading = *l,
            Op::TextFont { name, size } => {
                cur_font = Some(name.as_str().to_string());
                cur_size = *size;
            }
            Op::TextDraw { text } => {
                let info = cur_font.as_ref().and_then(|n| fonts.get(n));
                let decoded = decode_bytes(text.as_bytes(), info);
                if !decoded.trim().is_empty() {
                    let (ux, uy) = (text_matrix.e, text_matrix.f);
                    let (px, py) = to_page(&ctm, ux, uy);
                    words.push(Word { x0: px, y: py, text: decoded.clone() });
                }
                let adv = decoded.chars().count() as f32 * cur_size * 0.5;
                text_matrix = mat_mul(&text_matrix, &translate(adv / cur_size.max(1.0), 0.0));
            }
            Op::TextDrawAdjusted { array } => {
                let info = cur_font.as_ref().and_then(|n| fonts.get(n));
                let mut run = String::new();
                let (sx, sy) = {
                    let (px, py) = to_page(&ctm, text_matrix.e, text_matrix.f);
                    (px, py)
                };
                for item in array {
                    match item {
                        TextDrawAdjusted::Text(t) => {
                            run.push_str(&decode_bytes(t.as_bytes(), info));
                        }
                        TextDrawAdjusted::Spacing(o) => {
                            let adj = -o / 1000.0 * cur_size;
                            text_matrix = mat_mul(&text_matrix, &translate(adj / cur_size.max(1.0), 0.0));
                        }
                    }
                }
                if !run.trim().is_empty() {
                    words.push(Word { x0: sx, y: sy, text: run });
                }
            }
            // ----- paths -----
            Op::MoveTo { p } => {
                let pt = to_page(&ctm, p.x, p.y);
                path_pts.clear();
                path_pts.push(pt);
                sub_start = pt;
                cur_pt = pt;
            }
            Op::LineTo { p } => {
                let pt = to_page(&ctm, p.x, p.y);
                segments.push(Segment { x1: cur_pt.0, y1: cur_pt.1, x2: pt.0, y2: pt.1 });
                path_pts.push(pt);
                cur_pt = pt;
            }
            Op::Rect { rect } => {
                let a = to_page(&ctm, rect.x, rect.y);
                let b = to_page(&ctm, rect.x + rect.width, rect.y);
                let c = to_page(&ctm, rect.x + rect.width, rect.y + rect.height);
                let d = to_page(&ctm, rect.x, rect.y + rect.height);
                segments.push(Segment { x1: a.0, y1: a.1, x2: b.0, y2: b.1 });
                segments.push(Segment { x1: b.0, y1: b.1, x2: c.0, y2: c.1 });
                segments.push(Segment { x1: c.0, y1: c.1, x2: d.0, y2: d.1 });
                segments.push(Segment { x1: d.0, y1: d.1, x2: a.0, y2: a.1 });
            }
            Op::Close => {
                segments.push(Segment { x1: cur_pt.0, y1: cur_pt.1, x2: sub_start.0, y2: sub_start.1 });
                cur_pt = sub_start;
            }
            _ => {}
        }
    }

    Ok(PageData { words, segments })
}

// ---------- table detection ----------

fn cluster(values: &mut Vec<f32>, tol: f32) -> Vec<f32> {
    if values.is_empty() {
        return vec![];
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut clusters: Vec<Vec<f32>> = vec![];
    for &v in values.iter() {
        if let Some(last) = clusters.last_mut() {
            if (v - *last.last().unwrap()).abs() <= tol {
                last.push(v);
            } else {
                clusters.push(vec![v]);
            }
        } else {
            clusters.push(vec![v]);
        }
    }
    clusters.iter().map(|c| c.iter().sum::<f32>() / c.len() as f32).collect()
}

struct Grid {
    cols: Vec<f32>,
}

fn detect_grid(page: &PageData) -> Option<Grid> {
    let min_len = 15.0;
    let mut h_ys = vec![];
    let mut v_xs = vec![];
    for s in &page.segments {
        let dx = (s.x2 - s.x1).abs();
        let dy = (s.y2 - s.y1).abs();
        if dy <= 1.0 && dx >= min_len {
            h_ys.push((s.y1 + s.y2) / 2.0);
        } else if dx <= 1.0 && dy >= min_len {
            v_xs.push((s.x1 + s.x2) / 2.0);
        }
    }
    let rows = cluster(&mut h_ys, 2.0);
    let cols = cluster(&mut v_xs, 2.0);
    if rows.len() >= 2 && cols.len() >= 2 {
        Some(Grid { cols })
    } else {
        None
    }
}

// ---------- public entry ----------

fn floor_char_boundary(s: &str, idx: usize) -> usize {
    let mut i = idx.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

pub fn parse(path: &str) -> Result<Vec<StandardEntry>, String> {
    let file = FileOptions::cached()
        .open(path)
        .map_err(|e| format!("打开PDF失败：{}", e))?;
    let resolver = file.resolver();

    let std_re = Regex::new(
        r"([A-Za-z]+[/]?[A-Za-z]*)\s*([0-9]+(?:[.\-][0-9]+)*)\s*[-\u{FF0D}\u{2014}]\s*([0-9]{4})",
    )
    .unwrap();
    let noise_re = Regex::new(r"[A-Za-z]*\d{6,}").unwrap();

    let mut entries: Vec<StandardEntry> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (page_idx, page_res) in file.pages().enumerate() {
        let page_num = (page_idx + 1) as u32;
        let page = match page_res {
            Ok(p) => p,
            Err(_) => continue,
        };
        let data = match extract_page(&page, &resolver) {
            Ok(d) => d,
            Err(_) => continue,
        };

        if let Some(grid) = detect_grid(&data) {
            // table mode: concatenate each column and extract standards.
            // Per-column concatenation heals codes split across rows (vertically merged cells).
            let bracket_re = Regex::new(r"《([^》]+)》").unwrap();
            for ci in 0..grid.cols.len() - 1 {
                let x_left = grid.cols[ci];
                let x_right = grid.cols[ci + 1];
                let mut col_words: Vec<&Word> = data
                    .words
                    .iter()
                    .filter(|w| w.x0 >= x_left && w.x0 < x_right)
                    .collect();
                if col_words.is_empty() {
                    continue;
                }
                col_words.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap().then(a.x0.partial_cmp(&b.x0).unwrap()));

                let mut text = String::new();
                let mut last_y = f32::NAN;
                for w in &col_words {
                    if !last_y.is_nan() && (w.y - last_y).abs() > 2.0 {
                        text.push('\n');
                    }
                    text.push_str(&w.text);
                    last_y = w.y;
                }

                let mut prev_end = 0usize;
                for m in std_re.captures_iter(&text) {
                    let mstart = m.get(0).unwrap().start();
                    let mend = m.get(0).unwrap().end();
                    let code = format!("{} {}-{}", m[1].replace(' ', ""), &m[2], &m[3]);
                    let norm = standard_parser::normalize(&code);

                    let name = {
                        let after_end = floor_char_boundary(&text, mend + 150);
                        let after = &text[mend..after_end];
                        if let Some(b) = bracket_re.captures(after) {
                            b[1].replace('\n', "").trim().to_string()
                        } else {
                            let bs = floor_char_boundary(&text, mstart.saturating_sub(150));
                            let before = &text[bs..mstart];
                            if let Some(b) = bracket_re.captures_iter(before).last() {
                                b[1].replace('\n', "").trim().to_string()
                            } else {
                                let seg = text[prev_end..mstart].replace('\n', "");
                                let seg = match noise_re.find_iter(&seg).last() {
                                    Some(nm) => seg[nm.end()..].to_string(),
                                    None => seg,
                                };
                                seg.trim_matches(|c: char| c == '、' || c == '，' || c == ',' || c.is_whitespace())
                                    .to_string()
                            }
                        }
                    };
                    prev_end = mend;

                    if !seen.insert(norm) {
                        continue;
                    }
                    entries.push(StandardEntry { code, name, page: Some(page_num), sheet: String::new() });
                }
            }
        } else {
            // fallback: flat text in reading order
            let bracket_re = Regex::new(r"《([^》]+)》").unwrap();
            let mut flat = String::new();
            let mut ws: Vec<&Word> = data.words.iter().collect();
            ws.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap().then(a.x0.partial_cmp(&b.x0).unwrap()));
            let mut last_y = f32::NAN;
            for w in ws {
                if !last_y.is_nan() && (w.y - last_y).abs() > 2.0 {
                    flat.push('\n');
                }
                flat.push_str(&w.text);
                last_y = w.y;
            }
            let mut prev_end = 0usize;
            for m in std_re.captures_iter(&flat) {
                let mstart = m.get(0).unwrap().start();
                let mend = m.get(0).unwrap().end();
                let code = format!("{} {}-{}", m[1].replace(' ', ""), &m[2], &m[3]);
                let norm = standard_parser::normalize(&code);
                let name = {
                    let after_end = floor_char_boundary(&flat, mend + 150);
                    let after = &flat[mend..after_end];
                    if let Some(b) = bracket_re.captures(after) {
                        b[1].replace('\n', "").trim().to_string()
                    } else {
                        let seg = flat[prev_end..mstart].replace('\n', "");
                        let seg = match noise_re.find_iter(&seg).last() {
                            Some(nm) => seg[nm.end()..].to_string(),
                            None => seg,
                        };
                        seg.trim_matches(|c: char| c == '、' || c == '，' || c == ',' || c.is_whitespace())
                            .to_string()
                    }
                };
                prev_end = mend;
                if !seen.insert(norm) {
                    continue;
                }
                entries.push(StandardEntry { code, name, page: Some(page_num), sheet: String::new() });
            }
        }
    }

    Ok(entries)
}

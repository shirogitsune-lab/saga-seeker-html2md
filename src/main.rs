use encoding_rs::SHIFT_JIS;
use html_to_markdown_rs::{convert, ConversionOptions};
use kuchiki::traits::*;
use serde_json::Value;
use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const TOOL_NAME: &str = "saga-seeker-html2md";
const REMOVED_SECTION_TITLES: &[&str] = &["アイコン画像"];
const FILTERED_ITEM_SECTIONS: &[&str] = &["スキル", "性格キーワード", "思い出"];
const STATUS_TITLE: &str = "ステータス";
const STATUS_NAMES: &[&str] = &["筋力", "耐久力", "知力", "精神力", "素早さ", "運"];
const STATUS_BOUNDARY_NAMES: &[&str] =
    &["筋力", "耐久力", "知力", "精神力", "素早さ", "運", "魅力"];
const STATUS_RANKS: &[&str] = &["EX", "S", "A", "B", "C", "D", "E"];
const CHARACTER_DETAIL_TITLE: &str = "キャラクター詳細";
const CHARACTER_DETAIL_SUBHEADINGS: &[&str] = &[
    "基本設定",
    "外見",
    "性格",
    "口調",
    "経歴",
    "特技と役割",
    "その他の特徴",
];

#[derive(Debug)]
struct FileResult {
    input: PathBuf,
    output: Option<PathBuf>,
    error: Option<String>,
    timings: Vec<StageTiming>,
}

#[derive(Debug)]
struct RunSummary {
    total: usize,
    success: usize,
    failure: usize,
}

#[derive(Debug)]
struct StageTiming {
    name: &'static str,
    duration: Duration,
}

#[derive(Debug)]
struct Section {
    heading: String,
    body: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SkillInfo {
    name: String,
    description: Option<String>,
}

fn main() {
    let mut log_lines = Vec::new();
    println!("処理開始");

    let completion_message = match run(&mut log_lines) {
        Ok(summary) => completion_message(&summary),
        Err(err) => {
            log_lines.push(format!("[FATAL] {err}"));
            "完了: エラーが発生しました。詳細は log.txt を確認してください。".to_string()
        }
    };

    if let Err(err) = write_log(&log_lines) {
        let _ = err;
    }

    println!("{completion_message}");
    wait_for_enter();
}

fn run(log_lines: &mut Vec<String>) -> Result<RunSummary, Box<dyn Error>> {
    let run_started = Instant::now();
    let base_dir = exe_dir()?;
    let input_dir = base_dir.join("input");
    let output_dir = base_dir.join("output");

    fs::create_dir_all(&input_dir)?;
    fs::create_dir_all(&output_dir)?;

    let collect_started = Instant::now();
    let files = collect_html_files(&input_dir)?;
    let collect_duration = collect_started.elapsed();
    let total = files.len();

    log_lines.push(format!("ツール名: {TOOL_NAME}"));
    log_lines.push(format!("バージョン: {}", env!("CARGO_PKG_VERSION")));
    log_lines.push(format!(
        "実行日時: {}",
        format_system_time(SystemTime::now())
    ));
    log_lines.push(format!("実行ディレクトリ: {}", base_dir.display()));
    log_lines.push(format!("input フォルダ: {}", input_dir.display()));
    log_lines.push(format!("output フォルダ: {}", output_dir.display()));
    log_lines.push(format!("変換対象ファイル数: {total}"));
    log_lines.push(format!(
        "ファイル一覧収集: {}",
        format_duration(collect_duration)
    ));

    let mut results = Vec::new();
    for file in files {
        let result = convert_file(&file, &output_dir);
        append_file_result_log(log_lines, &result);
        results.push(result);
    }

    let success = results
        .iter()
        .filter(|result| result.error.is_none())
        .count();
    let failure = total.saturating_sub(success);

    log_lines.push(String::new());
    log_lines.push(format!("合計件数: {total}"));
    log_lines.push(format!("成功件数: {success}"));
    log_lines.push(format!("失敗件数: {failure}"));
    log_lines.push(format!(
        "合計時間: {}",
        format_duration(run_started.elapsed())
    ));

    Ok(RunSummary {
        total,
        success,
        failure,
    })
}

fn exe_dir() -> Result<PathBuf, Box<dyn Error>> {
    let exe = env::current_exe()?;
    Ok(exe
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or(env::current_dir()?))
}

fn collect_html_files(input_dir: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut files = Vec::new();

    for entry in fs::read_dir(input_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let path = entry.path();
        if is_html_file(&path) {
            files.push(path);
        }
    }

    files.sort_by_key(|path| path.file_name().map(OsStr::to_os_string));
    Ok(files)
}

fn is_html_file(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm"))
        .unwrap_or(false)
}

fn convert_file(input: &Path, output_dir: &Path) -> FileResult {
    let total_started = Instant::now();
    let mut timings = Vec::new();

    let stage_started = Instant::now();
    let bytes = match fs::read(input) {
        Ok(bytes) => bytes,
        Err(err) => {
            timings.push(StageTiming {
                name: "ファイル読み込み",
                duration: stage_started.elapsed(),
            });
            return file_error(input, None, err, timings, total_started);
        }
    };
    timings.push(StageTiming {
        name: "ファイル読み込み",
        duration: stage_started.elapsed(),
    });

    let stage_started = Instant::now();
    let html = match decode_html(&bytes) {
        Ok(html) => html,
        Err(err) => {
            timings.push(StageTiming {
                name: "文字コード判定・デコード",
                duration: stage_started.elapsed(),
            });
            return file_error(input, None, err, timings, total_started);
        }
    };
    timings.push(StageTiming {
        name: "文字コード判定・デコード",
        duration: stage_started.elapsed(),
    });

    let stage_started = Instant::now();
    let document = kuchiki::parse_html().one(html.as_str());
    let skills = extract_skills(&document);
    let cleaned_html = preprocess_html(&document, &html);
    timings.push(StageTiming {
        name: "HTMLパース・スキル抽出・前処理",
        duration: stage_started.elapsed(),
    });

    let options = ConversionOptions {
        extract_images: false,
        extract_metadata: false,
        include_document_structure: false,
        ..Default::default()
    };

    let stage_started = Instant::now();
    let result = match convert(&cleaned_html, Some(options)) {
        Ok(result) => result,
        Err(err) => {
            timings.push(StageTiming {
                name: "Markdown変換",
                duration: stage_started.elapsed(),
            });
            return file_error(input, None, err, timings, total_started);
        }
    };
    timings.push(StageTiming {
        name: "Markdown変換",
        duration: stage_started.elapsed(),
    });

    let markdown = result.content.unwrap_or_default();

    let stage_started = Instant::now();
    let markdown = cleanup_markdown(&markdown, &skills);
    timings.push(StageTiming {
        name: "Markdown後処理",
        duration: stage_started.elapsed(),
    });

    let stem = match input.file_stem().and_then(OsStr::to_str) {
        Some(stem) => stem,
        None => {
            return file_error(
                input,
                None,
                "Could not determine output file name",
                timings,
                total_started,
            );
        }
    };
    let output = output_dir.join(format!("{stem}.md"));

    let stage_started = Instant::now();
    if let Err(err) = fs::write(&output, markdown.as_bytes()) {
        timings.push(StageTiming {
            name: "ファイル書き込み",
            duration: stage_started.elapsed(),
        });
        return file_error(input, Some(output), err, timings, total_started);
    }
    timings.push(StageTiming {
        name: "ファイル書き込み",
        duration: stage_started.elapsed(),
    });
    timings.push(StageTiming {
        name: "ファイル合計時間",
        duration: total_started.elapsed(),
    });

    FileResult {
        input: input.to_path_buf(),
        output: Some(output),
        error: None,
        timings,
    }
}

fn decode_html(bytes: &[u8]) -> Result<String, Box<dyn Error>> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Ok(std::str::from_utf8(&bytes[3..])?.to_string());
    }

    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(text.to_string()),
        Err(_) => {
            let (decoded, _, _) = SHIFT_JIS.decode(bytes);
            Ok(decoded.into_owned())
        }
    }
}

fn preprocess_html(document: &kuchiki::NodeRef, original_html: &str) -> String {
    remove_by_selector(document, "script");
    remove_by_selector(document, "style");
    remove_by_selector(document, "img");
    remove_by_selector(document, "picture");
    remove_by_selector(document, "svg");
    remove_by_selector(document, "canvas");
    remove_by_selector(document, "footer");
    remove_by_selector(document, "[hidden]");
    remove_hidden_by_inline_style(document);

    let mut output = Vec::new();
    if document.serialize(&mut output).is_err() {
        return original_html.to_string();
    }

    String::from_utf8(output).unwrap_or_else(|_| original_html.to_string())
}

fn remove_by_selector(document: &kuchiki::NodeRef, selector: &str) {
    if let Ok(nodes) = document.select(selector) {
        let nodes: Vec<_> = nodes.collect();
        for node in nodes {
            node.as_node().detach();
        }
    }
}

fn remove_hidden_by_inline_style(document: &kuchiki::NodeRef) {
    let Ok(nodes) = document.select("[style]") else {
        return;
    };

    let nodes: Vec<_> = nodes.collect();
    for node in nodes {
        let attrs = node.attributes.borrow();
        let Some(style) = attrs.get("style") else {
            continue;
        };

        let normalized = style.to_ascii_lowercase().replace(char::is_whitespace, "");
        if normalized.contains("display:none") || normalized.contains("visibility:hidden") {
            node.as_node().detach();
        }
    }
}

fn extract_skills(document: &kuchiki::NodeRef) -> Vec<SkillInfo> {
    let mut skills = extract_skills_from_html_attrs(document);
    merge_missing_skills(&mut skills, extract_skills_from_json(document));
    skills
}

fn extract_skills_from_html_attrs(document: &kuchiki::NodeRef) -> Vec<SkillInfo> {
    let Ok(nodes) = document.select("[data-skill-name]") else {
        return Vec::new();
    };

    nodes
        .filter_map(|node| {
            let attrs = node.attributes.borrow();
            let name = attrs.get("data-skill-name")?;
            let skill_type = attrs.get("data-skill-type").unwrap_or("");
            let description = attrs.get("data-skill-description").unwrap_or("");
            make_skill_info(name, skill_type, description)
        })
        .collect()
}

fn extract_skills_from_json(document: &kuchiki::NodeRef) -> Vec<SkillInfo> {
    let Ok(nodes) = document.select("script[type=\"application/json\"]") else {
        return Vec::new();
    };

    let mut skills = Vec::new();
    for node in nodes {
        let text = node.text_contents();
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        collect_skills_from_value(&value, &mut skills);
    }

    skills
}

fn collect_skills_from_value(value: &Value, skills: &mut Vec<SkillInfo>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "skills" {
                    if let Value::Array(items) = child {
                        for item in items {
                            if let Some(skill) = skill_from_json_item(item) {
                                skills.push(skill);
                            }
                        }
                    }
                } else if key != "魅力" {
                    collect_skills_from_value(child, skills);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_skills_from_value(item, skills);
            }
        }
        _ => {}
    }
}

fn skill_from_json_item(value: &Value) -> Option<SkillInfo> {
    let map = value.as_object()?;
    let name = string_field(map, &["name", "skillName", "title", "label"])?;
    let skill_type = string_field(map, &["type", "skillType", "category"]).unwrap_or_default();
    let description = string_field(map, &["description", "skillDescription", "detail", "text"])
        .unwrap_or_default();

    make_skill_info(&name, &skill_type, &description)
}

fn string_field(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| map.get(*key))
        .filter_map(Value::as_str)
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn make_skill_info(name: &str, _skill_type: &str, description: &str) -> Option<SkillInfo> {
    let name = normalize_text(name);
    let description = normalize_text(description);

    if name.is_empty() || name == "空白" || name == "魅力" {
        return None;
    }

    let description = if description.is_empty() || description == "空白" || description == "無し"
    {
        None
    } else {
        Some(description)
    };

    Some(SkillInfo { name, description })
}

fn normalize_text(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn merge_missing_skills(skills: &mut Vec<SkillInfo>, fallback: Vec<SkillInfo>) {
    for fallback_skill in fallback {
        if let Some(existing) = skills
            .iter_mut()
            .find(|skill| skill.name == fallback_skill.name)
        {
            if existing.description.is_none() && fallback_skill.description.is_some() {
                existing.description = fallback_skill.description;
            }
        } else {
            skills.push(fallback_skill);
        }
    }
}

fn cleanup_markdown(markdown: &str, skills: &[SkillInfo]) -> String {
    let lines = initial_line_filter(markdown);
    let (preamble, sections) = split_sections(&lines);
    let mut output = filter_preamble(&preamble);

    for section in sections {
        if let Some(mut section_lines) = process_section(section, skills) {
            if !output.is_empty() && !output.last().is_some_and(|line| line.is_empty()) {
                output.push(String::new());
            }
            output.append(&mut section_lines);
        }
    }

    normalize_blank_lines(&output)
}

fn initial_line_filter(markdown: &str) -> Vec<String> {
    markdown
        .lines()
        .map(str::trim_end)
        .filter(|line| !is_image_line(line))
        .filter(|line| !is_footer_line(line))
        .map(ToOwned::to_owned)
        .collect()
}

fn filter_preamble(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| !should_skip_noise_line(line))
        .cloned()
        .collect()
}

fn split_sections(lines: &[String]) -> (Vec<String>, Vec<Section>) {
    let mut preamble = Vec::new();
    let mut sections = Vec::new();
    let mut current: Option<Section> = None;

    for line in lines {
        if is_h2_heading(line) {
            if let Some(section) = current.take() {
                sections.push(section);
            }
            current = Some(Section {
                heading: line.clone(),
                body: Vec::new(),
            });
        } else if let Some(section) = current.as_mut() {
            section.body.push(line.clone());
        } else {
            preamble.push(line.clone());
        }
    }

    if let Some(section) = current {
        sections.push(section);
    }

    (preamble, sections)
}

fn process_section(section: Section, skills: &[SkillInfo]) -> Option<Vec<String>> {
    let title = heading_title(&section.heading);
    if REMOVED_SECTION_TITLES.contains(&title) || title == "魅力" {
        return None;
    }

    if title == STATUS_TITLE {
        return process_status_section(&section.body).map(|mut body| {
            let mut lines = vec![section.heading, String::new()];
            lines.append(&mut body);
            lines
        });
    }

    if title == CHARACTER_DETAIL_TITLE {
        return process_character_detail_section(section);
    }

    if title == "スキル" && !skills.is_empty() {
        return Some(render_skill_section(section.heading, skills));
    }

    let body = if FILTERED_ITEM_SECTIONS.contains(&title) {
        filter_empty_items(&section.body)
    } else {
        section
            .body
            .into_iter()
            .filter(|line| !should_skip_noise_line(line))
            .collect()
    };

    if section_is_empty(&body) {
        return None;
    }

    let mut lines = vec![section.heading, String::new()];
    lines.extend(trim_outer_blank_lines(body));
    Some(lines)
}

fn process_character_detail_section(section: Section) -> Option<Vec<String>> {
    let mut body = Vec::new();
    let mut skipping_charm = false;

    for line in section.body {
        let trimmed = line.trim();

        if is_character_detail_charm_heading(trimmed) {
            skipping_charm = true;
            continue;
        }

        if is_character_detail_valid_heading(trimmed) {
            skipping_charm = false;
            if trimmed.starts_with("### ") {
                body.push(line);
            } else {
                body.push(format!("### {trimmed}"));
            }
            continue;
        }

        if skipping_charm || should_skip_noise_line(trimmed) {
            continue;
        }

        body.push(line);
    }

    body = remove_empty_h3_sections(body);
    if section_is_empty(&body) {
        return None;
    }

    let mut lines = vec![section.heading, String::new()];
    lines.extend(trim_outer_blank_lines(body));
    Some(lines)
}

fn is_character_detail_valid_heading(line: &str) -> bool {
    let title = line.trim_start_matches('#').trim();
    CHARACTER_DETAIL_SUBHEADINGS.contains(&title)
}

fn is_character_detail_charm_heading(line: &str) -> bool {
    let title = line.trim_start_matches('#').trim();
    title == "魅力"
}

fn remove_empty_h3_sections(lines: Vec<String>) -> Vec<String> {
    let mut output = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut current_body = Vec::new();

    for line in lines {
        if line.trim_start().starts_with("### ") {
            flush_h3_section(&mut output, current_heading.take(), &mut current_body);
            current_heading = Some(line);
        } else if current_heading.is_some() {
            current_body.push(line);
        } else {
            output.push(line);
        }
    }

    flush_h3_section(&mut output, current_heading, &mut current_body);
    output
}

fn flush_h3_section(output: &mut Vec<String>, heading: Option<String>, body: &mut Vec<String>) {
    let Some(heading) = heading else {
        return;
    };

    let cleaned = trim_outer_blank_lines(std::mem::take(body));
    if section_is_empty(&cleaned) {
        return;
    }

    if !output.is_empty() && !output.last().is_some_and(|line| line.is_empty()) {
        output.push(String::new());
    }
    output.push(heading);
    output.push(String::new());
    output.extend(cleaned);
}

fn render_skill_section(heading: String, skills: &[SkillInfo]) -> Vec<String> {
    let mut lines = vec![heading, String::new()];

    for skill in skills {
        if let Some(description) = &skill.description {
            lines.push(format!("### {}", skill.name));
            lines.push(String::new());
            lines.extend(description.lines().map(ToOwned::to_owned));
            lines.push(String::new());
        } else {
            lines.push(format!("- {}", skill.name));
        }
    }

    trim_outer_blank_lines(lines)
}

fn process_status_section(lines: &[String]) -> Option<Vec<String>> {
    let mut result = Vec::new();

    for name in STATUS_NAMES {
        let start = lines
            .iter()
            .position(|line| line_contains_stat_name(line, name));
        let Some(start) = start else {
            continue;
        };

        let end = STATUS_BOUNDARY_NAMES
            .iter()
            .filter(|boundary_name| **boundary_name != *name)
            .filter_map(|next_name| {
                lines[start + 1..]
                    .iter()
                    .position(|line| line_contains_stat_name(line, next_name))
                    .map(|offset| start + 1 + offset)
            })
            .min()
            .unwrap_or(lines.len());

        if let Some(rank) = extract_rank(&lines[start..end]) {
            result.push(format!("- {name}: {rank}"));
        }
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

fn extract_rank(lines: &[String]) -> Option<&'static str> {
    for line in lines.iter().rev() {
        let cleaned = clean_marker_text(line);
        for rank in STATUS_RANKS {
            if cleaned == *rank || cleaned.ends_with(rank) {
                return Some(rank);
            }
        }
    }

    None
}

fn line_contains_stat_name(line: &str, name: &str) -> bool {
    let cleaned = clean_marker_text(line);
    cleaned == name || cleaned.starts_with(&format!("{name}:")) || cleaned.starts_with(name)
}

fn filter_empty_items(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| !is_empty_item(line))
        .filter(|line| !should_skip_noise_line(line))
        .cloned()
        .collect()
}

fn section_is_empty(lines: &[String]) -> bool {
    lines.iter().all(|line| {
        let trimmed = line.trim();
        trimmed.is_empty() || is_empty_item(trimmed) || is_table_separator(trimmed)
    })
}

fn should_skip_noise_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    is_empty_heading(trimmed)
        || is_empty_item(trimmed)
        || is_charm_line(trimmed)
        || is_image_line(trimmed)
        || is_footer_line(trimmed)
        || is_useless_status_marker(trimmed)
        || is_empty_table_row(trimmed)
}

fn is_h2_heading(line: &str) -> bool {
    line.starts_with("## ") && !line.starts_with("### ")
}

fn heading_title(line: &str) -> &str {
    line.trim_start_matches('#').trim()
}

fn is_empty_heading(line: &str) -> bool {
    let hashes = line.chars().take_while(|ch| *ch == '#').count();
    hashes > 0 && line[hashes..].trim().is_empty()
}

fn is_empty_item(line: &str) -> bool {
    let cleaned = clean_marker_text(line);
    cleaned.is_empty() || cleaned == "-" || cleaned == "*" || cleaned == "空白"
}

fn is_charm_line(line: &str) -> bool {
    let cleaned = clean_marker_text(line);
    cleaned == "魅力" || cleaned.starts_with("魅力:") || cleaned.starts_with("魅力：")
}

fn is_useless_status_marker(line: &str) -> bool {
    let cleaned = clean_marker_text(line);
    !cleaned.is_empty()
        && (cleaned.chars().all(|ch| ch == '*') || cleaned.chars().all(|ch| ch == '＊'))
}

fn clean_marker_text(line: &str) -> String {
    line.trim()
        .trim_start_matches(|ch| {
            matches!(ch, '-' | '*' | '＊' | '+' | '・') || ch.is_ascii_digit() || ch == '.'
        })
        .trim()
        .trim_matches('*')
        .trim_matches('＊')
        .trim()
        .to_string()
}

fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|')
        && trimmed.ends_with('|')
        && trimmed
            .chars()
            .all(|ch| matches!(ch, '|' | '-' | ':' | ' ' | '\t'))
}

fn is_empty_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    if !(trimmed.starts_with('|') && trimmed.ends_with('|')) {
        return false;
    }

    trimmed
        .split('|')
        .map(str::trim)
        .all(|cell| cell.is_empty())
}

fn is_image_line(line: &str) -> bool {
    let lowered = line.to_ascii_lowercase();
    lowered.contains("![")
        || lowered.contains("<img")
        || lowered.contains("data:image")
        || lowered.contains(";base64")
}

fn is_footer_line(line: &str) -> bool {
    let lowered = line.to_ascii_lowercase();
    line.contains("Steamストア")
        || line.contains("お問い合わせ")
        || line.contains("X(Twitter)")
        || line.contains("Discord")
        || line.contains("©Dagdoria Studio")
        || line.contains("©Mare Inc.")
        || lowered.contains("dagdoria studio")
        || lowered.contains("mare inc")
        || lowered.contains("twitter.com")
        || lowered.contains("discord")
        || lowered.contains("store.steampowered.com")
}

fn trim_outer_blank_lines(mut lines: Vec<String>) -> Vec<String> {
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    lines
}

fn normalize_blank_lines(lines: &[String]) -> String {
    let mut cleaned = Vec::new();
    let mut previous_blank = false;

    for line in lines {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            if !previous_blank && !cleaned.is_empty() {
                cleaned.push(String::new());
            }
            previous_blank = true;
            continue;
        }

        cleaned.push(trimmed.to_string());
        previous_blank = false;
    }

    while cleaned.last().is_some_and(|line| line.is_empty()) {
        cleaned.pop();
    }

    if cleaned.is_empty() {
        String::new()
    } else {
        format!("{}\n", cleaned.join("\n"))
    }
}

fn file_error(
    input: &Path,
    output: Option<PathBuf>,
    err: impl ToString,
    mut timings: Vec<StageTiming>,
    total_started: Instant,
) -> FileResult {
    timings.push(StageTiming {
        name: "ファイル合計時間",
        duration: total_started.elapsed(),
    });

    FileResult {
        input: input.to_path_buf(),
        output,
        error: Some(err.to_string()),
        timings,
    }
}

fn append_file_result_log(log_lines: &mut Vec<String>, result: &FileResult) {
    log_lines.push(String::new());
    if result.error.is_some() {
        log_lines.push(format!("[NG] {}", result.input.display()));
    } else {
        log_lines.push(format!("[OK] {}", result.input.display()));
    }

    log_lines.push(format!("入力ファイル名: {}", result.input.display()));
    match &result.output {
        Some(output) => log_lines.push(format!("出力ファイル名: {}", output.display())),
        None => log_lines.push("出力ファイル名: (not created)".to_string()),
    }

    if let Some(error) = &result.error {
        log_lines.push(format!("エラー詳細: {error}"));
    }

    for timing in &result.timings {
        log_lines.push(format!(
            "{}: {}",
            timing.name,
            format_duration(timing.duration)
        ));
    }
}

fn completion_message(summary: &RunSummary) -> String {
    if summary.total == 0 {
        "完了: 変換対象のHTMLファイルはありません。input フォルダに .html または .htm を入れてください。".to_string()
    } else {
        format!(
            "完了: 変換対象 {}件 / 成功 {}件 / 失敗 {}件。詳細は log.txt を確認してください。",
            summary.total, summary.success, summary.failure
        )
    }
}

fn format_system_time(time: SystemTime) -> String {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("{} seconds since Unix epoch", duration.as_secs()),
        Err(err) => format!("before Unix epoch by {} ms", err.duration().as_millis()),
    }
}

fn format_duration(duration: Duration) -> String {
    format!("{} ms", duration.as_millis())
}

fn write_log(lines: &[String]) -> Result<(), Box<dyn Error>> {
    let base_dir = exe_dir()?;
    let log_path = base_dir.join("log.txt");
    fs::write(log_path, lines.join("\n"))?;
    Ok(())
}

fn wait_for_enter() {
    print!("Enterキーで終了します...");
    let _ = io::stdout().flush();

    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
}

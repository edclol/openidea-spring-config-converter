mod converter;

use converter::{ConfigFormat, convert, detect};
use openidea_plugin::{Error, Invocation, NotificationLevel, Plugin, Position, Range, TextEdit, host, register_plugin};

#[derive(Default)]
pub struct SpringConfigConverter;

impl Plugin for SpringConfigConverter {
    fn run_command(&mut self, id: &str, ctx: &Invocation, _arg: Option<&str>) -> Result<Option<String>, Error> {
        let (target, file_mode) = match id {
            "to-yaml" | "file-to-yaml" => (ConfigFormat::Yaml, id.starts_with("file-")),
            "to-properties" | "file-to-properties" => (ConfigFormat::Properties, id.starts_with("file-")),
            "to-env" | "file-to-env" => (ConfigFormat::Env, id.starts_with("file-")),
            _ => return Err(Error::NotFound(format!("command {id}"))),
        };
        if file_mode {
            convert_file(ctx, target)?;
        } else {
            convert_selection(ctx, target)?;
        }
        Ok(None)
    }
}

register_plugin!(SpringConfigConverter);

fn convert_selection(ctx: &Invocation, target: ConfigFormat) -> Result<(), Error> {
    let editor = ctx.editor().ok_or("需要打开文件才能转换选区")?;
    let path = editor.path();
    let selections = editor.selections()?;
    for _ in 0..2 {
        let doc = editor.document()?;
        let mut edits = Vec::with_capacity(selections.len().max(1));
        if selections.iter().all(|range| range.is_empty()) {
            let range = whole_document(&doc.text);
            let source = detect(Some(&path), &doc.text);
            if source == target {
                return Err(format!("文件已是 {} 格式", target.label()).into());
            }
            let new_text = convert(&doc.text, source, target).map_err(Error::Failed)?;
            if new_text != doc.text {
                edits.push(TextEdit { range, text: new_text });
            }
        } else {
            for range in &selections {
                let Some(old) = doc.text_in(*range) else { continue };
                if old.trim().is_empty() {
                    continue;
                }
                let source = detect(Some(&path), old);
                if source == target {
                    continue;
                }
                let new_text = convert(old, source, target).map_err(Error::Failed)?;
                if new_text != old {
                    edits.push(TextEdit { range: *range, text: new_text });
                }
            }
        }
        if edits.is_empty() {
            return Err("选区中没有可转换的配置".into());
        }
        edits.sort_by_key(|edit| edit.range);
        edits.dedup_by(|next, prev| next.range.start < prev.range.end);
        match editor.apply_edits(doc.version, &edits) {
            Err(Error::Stale) => continue,
            other => {
                other?;
                host::notify(
                    NotificationLevel::Info,
                    "Spring 配置转换",
                    &format!("已将选区转换为 {}", target.label()),
                );
                return Ok(());
            }
        }
    }
    Err(Error::Stale)
}

fn convert_file(ctx: &Invocation, target: ConfigFormat) -> Result<(), Error> {
    let project = ctx.project().ok_or("需要打开项目才能转换文件")?;
    let path = ctx
        .selected_paths()
        .into_iter()
        .next()
        .or_else(|| ctx.editor().map(|editor| editor.path()))
        .ok_or("请先选中配置文件")?;
    let text = project.read_file(&path)?;
    let source = detect(Some(&path), &text);
    if source == target {
        return Err(format!("文件已是 {} 格式", target.label()).into());
    }
    let converted = convert(&text, source, target).map_err(Error::Failed)?;
    let target_path = with_extension(&path, target.extension());
    if project.exists(&target_path)? {
        return Err(format!("目标文件 {} 已存在，未覆盖", file_name(&target_path)).into());
    }
    project.write_file(&target_path, &converted)?;
    host::notify(
        NotificationLevel::Info,
        "Spring 配置转换",
        &format!("已生成 {}", file_name(&target_path)),
    );
    Ok(())
}

fn whole_document(text: &str) -> Range {
    let line = text.split('\n').count().saturating_sub(1) as u32;
    let last = text.rsplit('\n').next().unwrap_or("").encode_utf16().count() as u32;
    Range::new(Position::new(0, 0), Position::new(line, last))
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn with_extension(path: &str, extension: &str) -> String {
    let base = path.rsplit_once('.').map_or(path, |(base, _)| base);
    format!("{base}.{extension}")
}

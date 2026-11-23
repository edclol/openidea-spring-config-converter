use serde_yaml::{Mapping, Value as YamlValue};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigFormat {
    Yaml,
    Properties,
    Env,
}

impl ConfigFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Yaml => "yml",
            Self::Properties => "properties",
            Self::Env => "env",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Yaml => "YAML",
            Self::Properties => "Properties",
            Self::Env => "ENV",
        }
    }
}

#[derive(Clone, Debug)]
enum Value {
    Null,
    Str(String),
    Map(Vec<(String, Value)>),
    List(Vec<Value>),
}

pub fn detect(file_name: Option<&str>, content: &str) -> ConfigFormat {
    if let Some(name) = file_name {
        let name = name.to_lowercase();
        if name.ends_with(".yml") || name.ends_with(".yaml") {
            return ConfigFormat::Yaml;
        }
        if name.ends_with(".properties") {
            return ConfigFormat::Properties;
        }
        if name.ends_with(".env") {
            return ConfigFormat::Env;
        }
    }
    detect_from_content(content)
}

fn detect_from_content(text: &str) -> ConfigFormat {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    let kv_lines: Vec<&str> = lines.into_iter().filter(|line| is_kv_line(line)).collect();
    if !kv_lines.is_empty() {
        let has_dot = kv_lines.iter().any(|line| line.split('=').next().is_some_and(|key| key.contains('.')));
        return if has_dot { ConfigFormat::Properties } else { ConfigFormat::Env };
    }
    ConfigFormat::Yaml
}

fn is_kv_line(line: &str) -> bool {
    let Some(pos) = line.find('=') else { return false };
    if pos == 0 {
        return false;
    }
    line[..pos].chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | '[' | ']'))
}

pub fn convert(text: &str, from: ConfigFormat, to: ConfigFormat) -> Result<String, String> {
    let root = parse(text, from)?;
    if matches!(&root, Value::Map(map) if map.is_empty()) {
        return Err("未找到可转换的键值配置".to_string());
    }
    let normalized = normalize(root);
    render(&normalized, to)
}

fn parse(text: &str, format: ConfigFormat) -> Result<Value, String> {
    match format {
        ConfigFormat::Yaml => parse_yaml(text),
        ConfigFormat::Properties => parse_flat(text, '.', false, true),
        ConfigFormat::Env => parse_flat(text, '_', true, false),
    }
}

fn parse_yaml(text: &str) -> Result<Value, String> {
    let loaded: YamlValue = serde_yaml::from_str(text).map_err(|e| format!("YAML 解析失败: {e}"))?;
    if loaded.is_null() {
        return Ok(Value::Map(Vec::new()));
    }
    let YamlValue::Mapping(mapping) = loaded else {
        return Err("YAML 顶层必须是键值映射".to_string());
    };
    let mut result = Vec::with_capacity(mapping.len());
    for (key, value) in mapping {
        result.push((yaml_key(key)?, from_yaml_value(value)));
    }
    Ok(Value::Map(result))
}

fn yaml_key(key: YamlValue) -> Result<String, String> {
    match key {
        YamlValue::String(s) => Ok(s),
        YamlValue::Number(n) => Ok(n.to_string()),
        YamlValue::Bool(b) => Ok(b.to_string()),
        _ => Err("YAML 键必须是标量".to_string()),
    }
}

fn from_yaml_value(value: YamlValue) -> Value {
    match value {
        YamlValue::Null => Value::Null,
        YamlValue::Bool(b) => Value::Str(b.to_string()),
        YamlValue::Number(n) => Value::Str(n.to_string()),
        YamlValue::String(s) => Value::Str(s),
        YamlValue::Sequence(seq) => Value::List(seq.into_iter().map(from_yaml_value).collect()),
        YamlValue::Mapping(mapping) => Value::Map(
            mapping
                .into_iter()
                .filter_map(|(k, v)| yaml_key(k).ok().map(|key| (key, from_yaml_value(v))))
                .collect(),
        ),
        _ => Value::Str(String::new()),
    }
}

fn parse_flat(text: &str, separator: char, lowercase: bool, properties: bool) -> Result<Value, String> {
    let mut root = Vec::new();
    let mut has_entry = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let split = if properties { find_unescaped_eq(line) } else { line.find('=') };
        let Some(index) = split else {
            return Err(format!("无效的键值行: {line}"));
        };
        if index == 0 {
            return Err(format!("无效的键值行: {line}"));
        }
        has_entry = true;
        let key = line[..index].trim();
        let key = if lowercase { key.to_lowercase() } else { key.to_string() };
        let key = if properties { unescape_property(&key)? } else { key };
        let mut value = line[index + 1..].trim().to_string();
        if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
            value = value[1..value.len() - 1].to_string();
        }
        let value = if properties { unescape_property(&value)? } else { value };
        insert(&mut root, &split_key(&key, separator), value);
    }
    if !has_entry {
        return Err("未找到有效的 KEY=VALUE 行".to_string());
    }
    Ok(Value::Map(root))
}

/// 找到第一个未被反斜杠转义的 '='（Properties 键可含转义的 \=）。
fn find_unescaped_eq(line: &str) -> Option<usize> {
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
        } else if c == '=' {
            return Some(i);
        }
    }
    None
}

/// Java Properties 反转义：\n \r \t \f \\ 及其他字符按 Java 规则解析；
/// \uXXXX 解码为 Unicode 码点（含 UTF-16 代理对），畸形转义返回错误。
fn unescape_property(text: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut rest: std::iter::Peekable<std::str::Chars<'_>> = text.chars().peekable();
    while let Some(c) = rest.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(next) = rest.next() else {
            return Err("Properties 末尾存在孤立反斜杠".to_string());
        };
        match next {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'f' => out.push('\u{000C}'),
            '\\' => out.push('\\'),
            'u' => {
                let first = read_hex4(&mut rest, "u")?;
                let code = if (0xD800..=0xDBFF).contains(&first) {
                    // 高位代理：必须紧跟 \uDC00-\uDFFF 低位代理。
                    let after_high = rest.next();
                    let marker = rest.next();
                    if after_high != Some('\\') || marker != Some('u') {
                        return Err(format!("高位代理 {first:04X} 后应为 \\u，实际为 {after_high:?}{marker:?}"));
                    }
                    let second = read_hex4(&mut rest, "u")?;
                    if !(0xDC00..=0xDFFF).contains(&second) {
                        return Err(format!("无效的 Unicode 低位代理 {second:04X}"));
                    }
                    0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                } else if (0xDC00..=0xDFFF).contains(&first) {
                    return Err(format!("孤立的 Unicode 低位代理 {first:04X}"));
                } else {
                    first
                };
                let ch = char::from_u32(code)
                    .ok_or_else(|| format!("无效的 Unicode 码点 {code:04X}"))?;
                out.push(ch);
            }
            other => {
                // Java: 未知转义忽略反斜杠，但 = : 空格需保留（由调用方转义）。
                out.push(other);
            }
        }
    }
    Ok(out)
}

/// 读取 \u 后的四位十六进制数字。
fn read_hex4<I: Iterator<Item = char>>(chars: &mut std::iter::Peekable<I>, kind: &str) -> Result<u32, String> {
    let mut value = 0u32;
    for _ in 0..4 {
        let digit = chars
            .next()
            .ok_or_else(|| format!("\\{kind} 转义的十六进制位数不足"))?;
        let n = digit
            .to_digit(16)
            .ok_or_else(|| format!("\\{kind} 转义含非法十六进制字符 {digit:?}"))?;
        value = value * 16 + n;
    }
    Ok(value)
}

fn split_key(key: &str, separator: char) -> Vec<String> {
    key.split(separator)
        .filter(|segment| !segment.is_empty())
        .flat_map(|segment| match indexed_segment(segment) {
            Some((name, index)) => vec![name.to_string(), index.to_string()],
            None => vec![segment.to_string()],
        })
        .collect()
}

fn indexed_segment(segment: &str) -> Option<(&str, &str)> {
    if !segment.ends_with(']') {
        return None;
    }
    let open = segment.rfind('[')?;
    if open == 0 {
        return None;
    }
    let index = &segment[open + 1..segment.len() - 1];
    if index.is_empty() || !index.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((&segment[..open], index))
}

fn insert(node: &mut Vec<(String, Value)>, path: &[String], value: String) {
    let key = &path[0];
    if path.len() == 1 {
        set_entry(node, key.clone(), Value::Str(value));
        return;
    }
    let position = node.iter().position(|(name, _)| name == key);
    let child = match position {
        Some(index) => &mut node[index].1,
        None => {
            node.push((key.clone(), Value::Map(Vec::new())));
            &mut node.last_mut().unwrap().1
        }
    };
    if !matches!(child, Value::Map(_)) {
        *child = Value::Map(Vec::new());
    }
    if let Value::Map(map) = child {
        insert(map, &path[1..], value);
    }
}

fn set_entry(node: &mut Vec<(String, Value)>, key: String, value: Value) {
    match node.iter().position(|(name, _)| *name == key) {
        Some(index) => node[index].1 = value,
        None => node.push((key, value)),
    }
}

fn normalize(value: Value) -> Value {
    match value {
        Value::Map(map) => {
            let normalized: Vec<(String, Value)> =
                map.into_iter().map(|(key, value)| (key, normalize(value))).collect();
            let all_numeric = !normalized.is_empty()
                && normalized.iter().all(|(key, _)| !key.is_empty() && key.chars().all(|c| c.is_ascii_digit()));
            if all_numeric {
                let indices: Option<Vec<usize>> = normalized.iter().map(|(key, _)| key.parse().ok()).collect();
                if let Some(indices) = indices {
                    if indices == (0..indices.len()).collect::<Vec<_>>() {
                        return Value::List(normalized.into_iter().map(|(_, value)| value).collect());
                    }
                }
            }
            Value::Map(normalized)
        }
        Value::List(list) => Value::List(list.into_iter().map(normalize).collect()),
        other => other,
    }
}

fn render(root: &Value, format: ConfigFormat) -> Result<String, String> {
    match format {
        ConfigFormat::Yaml => {
            let scalar = coerce(root);
            serde_yaml::to_string(&scalar).map_err(|e| format!("YAML 生成失败: {e}"))
        }
        ConfigFormat::Properties | ConfigFormat::Env => {
            let mut entries = Vec::new();
            walk(&[], root, &mut entries, format == ConfigFormat::Env);
            if entries.is_empty() {
                return Err("未找到可转换的键值配置".to_string());
            }
            Ok(entries
                .into_iter()
                .map(|(key, value)| match format {
                    ConfigFormat::Properties => format!("{}={}", escape_property_key(&key), escape_property_value(&value)),
                    _ => format!("{key}={value}"),
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
    }
}

/// Java Properties 键：等号、冒号、空格与控制字符需转义。
fn escape_property_key(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    for c in key.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '=' => out.push_str("\\="),
            ':' => out.push_str("\\:"),
            ' ' => out.push_str("\\ "),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{000C}' => out.push_str("\\f"),
            _ => out.push(c),
        }
    }
    out
}

/// Java Properties 值：反斜杠与控制字符转义；行首空格需前置反斜杠以免被忽略。
fn escape_property_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut at_start = true;
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{000C}' => out.push_str("\\f"),
            ' ' if at_start => out.push_str("\\ "),
            _ => out.push(c),
        }
        at_start = false;
    }
    out
}

fn walk(path: &[String], value: &Value, out: &mut Vec<(String, String)>, env_style: bool) {
    match value {
        Value::Map(map) => {
            for (key, child) in map {
                let mut next = path.to_vec();
                next.push(key.clone());
                walk(&next, child, out, env_style);
            }
        }
        Value::List(list) => {
            for (index, item) in list.iter().enumerate() {
                let mut next = path.to_vec();
                if env_style || next.is_empty() {
                    next.push(index.to_string());
                } else {
                    let last = next.pop().unwrap();
                    next.push(format!("{last}[{index}]"));
                }
                walk(&next, item, out, env_style);
            }
        }
        Value::Null => {}
        Value::Str(text) => out.push((join_key(path, env_style), text.clone())),
    }
}

fn join_key(path: &[String], env_style: bool) -> String {
    path.iter()
        .map(|segment| {
            if env_style {
                segment.replace('.', "_").replace('-', "").to_uppercase()
            } else {
                segment.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(if env_style { "_" } else { "." })
}

fn coerce(value: &Value) -> YamlValue {
    match value {
        Value::Null => YamlValue::Null,
        Value::Map(map) => {
            let mut result = Mapping::new();
            for (key, child) in map {
                result.insert(YamlValue::String(key.clone()), coerce(child));
            }
            YamlValue::Mapping(result)
        }
        Value::List(list) => YamlValue::Sequence(list.iter().map(coerce).collect()),
        Value::Str(text) => match coerce_scalar(text) {
            Scalar::Bool(value) => YamlValue::Bool(value),
            Scalar::Number(value) => YamlValue::Number(value.into()),
            Scalar::Text(value) => YamlValue::String(value),
        },
    }
}

enum Scalar {
    Bool(bool),
    Number(i64),
    Text(String),
}

fn coerce_scalar(text: &str) -> Scalar {
    if text.eq_ignore_ascii_case("true") {
        return Scalar::Bool(true);
    }
    if text.eq_ignore_ascii_case("false") {
        return Scalar::Bool(false);
    }
    if text.len() > 1 && text.starts_with('0') {
        return Scalar::Text(text.to_string());
    }
    match text.parse::<i64>() {
        Ok(value) => Scalar::Number(value),
        Err(_) => Scalar::Text(text.to_string()),
    }
}

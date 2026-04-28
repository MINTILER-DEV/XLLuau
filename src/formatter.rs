use crate::compiler::Result;

pub fn format_luau(source: &str) -> Result<String> {
    Ok(ensure_trailing_newline(source))
}

pub fn format_source(source: &str) -> String {
    let mut formatted = Vec::new();
    let mut indent = 0usize;
    let mut previous_blank = false;

    for raw_line in source.lines() {
        let trimmed_end = raw_line.trim_end();
        let trimmed = trimmed_end.trim_start();

        if trimmed.is_empty() {
            if !previous_blank && !formatted.is_empty() {
                formatted.push(String::new());
            }
            previous_blank = true;
            continue;
        }

        let dedent_before = should_dedent_before(trimmed);
        if dedent_before {
            indent = indent.saturating_sub(1);
        }

        formatted.push(format!("{}{}", "    ".repeat(indent), trimmed));
        previous_blank = false;

        if should_indent_after(trimmed) {
            indent += 1;
        }
    }

    ensure_trailing_newline(&formatted.join("\n"))
}

fn ensure_trailing_newline(source: &str) -> String {
    if source.ends_with('\n') {
        source.to_string()
    } else {
        format!("{source}\n")
    }
}

fn should_dedent_before(line: &str) -> bool {
    starts_with_word(line, "end")
        || starts_with_word(line, "else")
        || starts_with_word(line, "elseif")
        || starts_with_word(line, "until")
        || starts_with_word(line, "case")
        || starts_with_word(line, "default")
        || starts_with_word(line, "then")
        || starts_with_word(line, "catch")
}

fn should_indent_after(line: &str) -> bool {
    let trimmed = line.trim();
    if starts_with_word(trimmed, "end")
        || starts_with_word(trimmed, "until")
        || starts_with_word(trimmed, "case")
        || starts_with_word(trimmed, "default")
        || starts_with_word(trimmed, "else")
        || starts_with_word(trimmed, "elseif")
        || starts_with_word(trimmed, "then")
        || starts_with_word(trimmed, "catch")
    {
        return starts_with_word(trimmed, "case")
            || starts_with_word(trimmed, "default")
            || starts_with_word(trimmed, "else")
            || starts_with_word(trimmed, "elseif")
            || starts_with_word(trimmed, "then")
            || starts_with_word(trimmed, "catch");
    }

    starts_with_word(trimmed, "if")
        || starts_with_word(trimmed, "switch")
        || starts_with_word(trimmed, "match")
        || starts_with_word(trimmed, "while")
        || starts_with_word(trimmed, "for")
        || starts_with_word(trimmed, "repeat")
        || starts_with_word(trimmed, "function")
        || starts_with_word(trimmed, "task function")
        || starts_with_word(trimmed, "object")
        || starts_with_word(trimmed, "enum")
        || starts_with_word(trimmed, "do")
        || starts_with_word(trimmed, "on")
        || starts_with_word(trimmed, "once")
        || starts_with_word(trimmed, "watch")
}

fn starts_with_word(line: &str, prefix: &str) -> bool {
    if !line.starts_with(prefix) {
        return false;
    }
    line[prefix.len()..]
        .chars()
        .next()
        .map(|ch| ch.is_whitespace() || matches!(ch, '|' | '('))
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::format_source;

    #[test]
    fn formats_nested_blocks() {
        let source = r#"
if ok then
print(value)
else
print("no")
end
"#;
        let formatted = format_source(source);
        assert_eq!(
            formatted,
            "if ok then\n    print(value)\nelse\n    print(\"no\")\nend\n"
        );
    }

    #[test]
    fn formats_signal_state_blocks() {
        let source = r#"
signal OnLoaded: (value: number)
state count: number = 0
watch count |old, new|
print(old, new)
end
on OnLoaded |value|
print(value)
end
"#;
        let formatted = format_source(source);
        assert_eq!(
            formatted,
            "signal OnLoaded: (value: number)\nstate count: number = 0\nwatch count |old, new|\n    print(old, new)\nend\non OnLoaded |value|\n    print(value)\nend\n"
        );
    }
}

use crate::compiler::Result;

pub fn format_luau(source: &str) -> Result<String> {
    Ok(if source.ends_with('\n') {
        source.to_string()
    } else {
        format!("{source}\n")
    })
}

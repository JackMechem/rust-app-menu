pub fn parse_hyprctl_focused_monitor(json: &str) -> Option<String> {
    let focused_block = json
        .split('{')
        .find(|block| block.contains("\"focused\": true") || block.contains("\"focused\":true"))?;
    let name_start = focused_block.find("\"name\":")?;
    let after = focused_block[name_start + 7..].trim_start();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

pub fn parse_sway_focused_output(json: &str) -> Option<String> {
    let focused_block = json
        .split('{')
        .find(|block| block.contains("\"focused\": true") || block.contains("\"focused\":true"))?;
    let name_start = focused_block.find("\"name\":")?;
    let after = focused_block[name_start + 7..].trim_start();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

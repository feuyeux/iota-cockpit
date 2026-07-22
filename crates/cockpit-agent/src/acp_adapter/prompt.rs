pub(super) fn strip_full_prompt_echo(output: &str, prompt: &str) -> Option<String> {
    if prompt.is_empty() {
        return None;
    }
    let prompt_offset = output.find(prompt)?;
    let before = output[..prompt_offset].trim_end_matches(['\r', '\n']);
    let after = output[prompt_offset + prompt.len()..].trim_start_matches(['\r', '\n']);
    let mut cleaned = String::with_capacity(before.len() + after.len());
    cleaned.push_str(before);
    cleaned.push_str(after);
    Some(cleaned)
}

pub(super) fn common_prefix_bytes(left: &str, right: &str) -> usize {
    left.as_bytes()
        .iter()
        .zip(right.as_bytes())
        .take_while(|(left, right)| left == right)
        .count()
}

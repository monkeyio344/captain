pub struct GenerateReadmeOpts {
    pub crate_name: String,
    pub input: String,
    pub header: Option<String>,
    pub footer: Option<String>,
}

pub fn generate(opts: GenerateReadmeOpts) -> String {
    // Generate header by replacing "{CRATE}" in the provided header template (empty if not configured)
    let header = opts
        .header
        .as_deref()
        .unwrap_or("")
        .replace("{CRATE}", &opts.crate_name);

    // The main template content, passed in via `opts.input`
    let template_content = opts.input;

    // Use provided footer or empty (no built-in defaults)
    let footer = opts
        .footer
        .as_deref()
        .unwrap_or("")
        .replace("{CRATE}", &opts.crate_name);

    // Combine header, template, and footer
    // Only add newlines between non-empty sections
    let mut parts = Vec::new();
    if !header.is_empty() {
        parts.push(header);
    }
    parts.push(template_content);
    if !footer.is_empty() {
        parts.push(footer);
    }

    parts.join("\n")
}

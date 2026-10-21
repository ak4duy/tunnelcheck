use serde::Serialize;

#[derive(Serialize)]
struct Field<'a> {
    name: &'a str,
    value: &'a str,
    inline: bool,
}

#[derive(Serialize)]
struct Embed<'a> {
    title: &'a str,
    color: u32,
    fields: Vec<Field<'a>>,
}

#[derive(Serialize)]
struct Message<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<&'a str>,
    embeds: [Embed<'a>; 1],
}

pub fn mention(raw: &str, message: &str) -> String {
    let raw = raw.trim_start_matches('@');
    if raw.chars().all(|c| c.is_ascii_digit()) {
        format!("<@{raw}> {message}")
    } else {
        format!("@{raw} {message}")
    }
}

pub fn discord(
    url: &str,
    mention: Option<&str>,
    title: &str,
    color: u32,
    fields: &[(&str, &str)],
) -> reqwest::Result<()> {
    let fields = fields
        .iter()
        .map(|&(name, value)| Field {
            name,
            value,
            inline: false,
        })
        .collect();
    reqwest::blocking::Client::new()
        .post(url)
        .json(&Message {
            content: mention,
            embeds: [Embed {
                title,
                color,
                fields,
            }],
        })
        .send()?
        .error_for_status()?;
    Ok(())
}

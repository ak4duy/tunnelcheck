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
    embeds: [Embed<'a>; 1],
}

pub fn discord(url: &str, title: &str, color: u32, fields: &[(&str, &str)]) -> reqwest::Result<()> {
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

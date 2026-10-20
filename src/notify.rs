use serde::Serialize;

#[derive(Serialize)]
struct Message<'a> {
    content: &'a str,
}

pub fn discord(url: &str, content: &str) -> reqwest::Result<()> {
    reqwest::blocking::Client::new()
        .post(url)
        .json(&Message { content })
        .send()?
        .error_for_status()?;
    Ok(())
}

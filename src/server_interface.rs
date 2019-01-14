use crate::errors::*;

// GET http://server.com/search/123 -> String(JSON) -> Vec<Struct>
// [{
//     name: "123",
//     url: jiji / id: 123
// }]

type Url = String;

#[derive(Deserialize)]
struct ServerResponse {
    pub name: String,
    pub url: Url,
}

pub fn search(keyword: &str, pick: Option<usize>) -> Result<Option<Url>> {
    // TODO: add keyword escaping to avoid injections
    let mut response: reqwest::Response = reqwest::get(&format!("http://server.com/search/{}", keyword)).chain_err(|| "server unreachable")?;
    let files: Vec<ServerResponse> = response.json().chain_err(|| "failed deserializing server response")?;
    for (i, file) in files.iter().enumerate() {
        println!("{:2}: {}", i, file.name);
    }

    if let Some(index) = pick {
        // Ok(files.get(index).chain_err()
        Ok(Some(files.get(index).chain_err(|| "index out of bounds")?.url.clone()))
    } else {
        Ok(None)
    }
}
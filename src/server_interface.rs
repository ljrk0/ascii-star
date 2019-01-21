use crate::errors::*;
use tempfile::NamedTempFile;
use std::io::copy;

// GET http://server.com/search/123 -> String(JSON) -> Vec<Struct>
// [{
//     name: "123",
//     url: jiji / id: 123
// }]

type Url = String;

const SERVER_URL: &str = "http://localhost:8000";

#[derive(Deserialize)]
struct ServerResponse {
    pub results: Vec<SearchResult>,
}

#[derive(Deserialize)]
struct SearchResult {
    artist: String,
    title: String,
    genre: Option<String>,
    path: String,
}

pub fn search(keyword: &str, pick: Option<usize>) -> Result<Option<Url>> {
    // TODO: add keyword escaping to avoid injections
    let mut response: reqwest::Response = reqwest::get(&format!("{}/search?q={}", SERVER_URL, keyword)).chain_err(|| "server unreachable")?;
    let result: ServerResponse = response.json().chain_err(|| "failed deserializing server response")?;

    if let Some(index) = pick {
        // Ok(files.get(index).chain_err()
        let path = &result.results.get(index).chain_err(|| "index out of bounds")?.path;
        Ok(Some(format!("{}/{}", SERVER_URL, path)))
    } else {
        for (i, file) in result.results.iter().enumerate() {
            if let Some(genre) = file.genre.as_ref() {
                println!("{number:2}: {title} - {artist} ({genre})", number = i, title = file.title, artist = file.artist, genre = genre);
            } else {
                println!("{number:2}: {title} - {artist}", number = i, title = file.title, artist = file.artist);
            }
        }

        Ok(None)
    }
}

pub fn download_file(url: String) -> Result<NamedTempFile> {
    let mut dest = NamedTempFile::new()
        .chain_err(|| "could not create temporary file")?;

    let mut response = reqwest::get(&url)
        .chain_err(|| "could not retrieve .txt from server")?;

    copy(&mut response, dest.as_file_mut())
        .chain_err(|| "could not write to .txt file")?;

    Ok(dest)
}

#![allow(dead_code)]

use crate::errors::*;
use std::collections::HashMap;
use reqwest::Response;
use std::io::Read;
use std::io::prelude::*;
use std::fs::File;
use regex::Regex;
use super::UrlContentProvider;
use youtube_dl::{YoutubeDl,YoutubeDlOutput,Format};



pub struct Youtube {
    /// The 11-character video id
    pub id: String,
    /// The duration of the streams in seconds
    length: u32,
    /// The available streams (containing both video and audio)
    streams: Vec<Stream>,
    /// The available only-video streams
    videostreams: Vec<Stream>,
    /// The available only-audio streams
    audiostreams: Vec<Stream>,
}

impl UrlContentProvider for Youtube {
    fn urls(&self) -> Vec<&str> {
        let r = self.audiostreams.iter()
            .chain(self.streams.iter())
            .map(|s: &Stream| -> &str {&s.url})
            .collect();
        eprintln!("{:#?}", r);
        return r;
    }
}

impl Youtube {
    /// Create a YtVideo from an url
    pub fn new(url: &str) -> Result<Self> {
        eprintln!("url: {}", url);
        let ytoutput = YoutubeDl::new(url).socket_timeout("15").run().unwrap();
        let sv = match ytoutput {
            YoutubeDlOutput::SingleVideo(sv) => *sv,
            _ => panic!("Only SingleVideo URLs supported")
        };
        //println!("{:#?}", sv);

        let mut audiostreams: Vec<Stream> = Vec::new();

        // [...] a video [...] must contain either a formats entry or a url one
        if let Some(url) = sv.url {
            let astream = Stream {
                extension: sv.ext.unwrap(),
                quality: sv.quality.unwrap_or_default().to_string(),
                url: url,
            };
            audiostreams.push(astream);
        } else {
            let formats = sv.formats.unwrap();
            for fmt in formats {
                if let Some(_acodec) = fmt.acodec {
                    let astream = Stream {
                        extension: fmt.ext.unwrap(),
                        quality: fmt.quality.unwrap_or_default().to_string(),
                        url: fmt.url.unwrap(),
                    };
                    audiostreams.push(astream);
                }
            }
        }

        let streams = Vec::new();
        let videostreams = Vec::new();
        Ok(Self {
            id: sv.id,
            length: sv.duration.unwrap().as_u64().unwrap() as u32,
            streams,
            audiostreams,
            videostreams,
        })
    }
}

/// A (audio/video) stream
#[derive(Debug, Clone)]
struct Stream {
    /// The extension of the stream
    pub extension: String,
    /// The quality of the stream
    pub quality: String,
    /// The url of the stream
    pub url: String,
}

impl Stream {
    /// Downloads the content stream from `Stream` object,
    /// saving it into `path`.
    pub fn download(&self, path: &str) -> Result<()> {
        let response = send_request(&self.url)?;
        let file_size = get_file_size(&response)?;
        let file_name = format!("{}.{}", path, &self.extension);
        write_file(response, &file_name, file_size)?;
        Ok(())
    }
}

// get file size from Content-Length header
fn get_file_size(response: &Response) -> Result<u64> {
    response.headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|length| length.to_str().ok())
        .and_then(|length| length.parse::<u64>().ok())
        .chain_err(|| "Content-Length header missing")
}

fn send_request(url: &str) -> Result<Response> {
    let res = reqwest::get(url);
    res.chain_err(|| format!("Error sending request to {}", url))
}

fn write_file(mut response: Response, title: &str, _file_size: u64) -> Result<()> {
    // let mut pb = ProgressBar::new(file_size);

    let mut buf = [0; 128 * 1024];
    let mut file = File::create(title).chain_err(|| format!("Error creating file: {}", title))?;
    loop {
        let len = response.read(&mut buf).chain_err(|| "Error reading from response")?;
        file.write_all(&buf[..len]).chain_err(|| format!("Error writing to file: {}", title))?;
        // pb.add(len as u64);
        if len == 0 {
            break;
        }
    }
    Ok(())
}

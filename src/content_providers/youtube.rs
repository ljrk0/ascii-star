use crate::errors::*;
use std::collections::HashMap;
use hyper::client::response::Response;
use hyper::Client;
use hyper::net::HttpsConnector;
use hyper::header::{ContentLength, Headers, ByteRangeSpec, Range};
use hyper_native_tls::NativeTlsClient;
use std::io::Read;
use std::io::prelude::*;
use std::fs::File;
use regex::Regex;
use super::UrlContentProvider;

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
        self.audiostreams.iter()
            .chain(self.streams.iter())
            .map(|s: &Stream| -> &str {&s.url})
            .collect()
    }
}

impl Youtube {
    /// Create a YtVideo from an url
    pub fn new(url: &str) -> Result<Self> {
        // Regex for youtube URLs
        let url_regex = Regex::new(r"^.*(?:(?:youtu\.be/|v/|vi/|u/w/|embed/)|(?:(?:watch)?\?v(?:i)?=|\&v(?:i)?=))([^#\&\?]*).*").unwrap();
        let mut vid = url;

        // TODO: is this equivalent? Should be
        // if url_regex.is_match(vid) {
        //     let vid_split = url_regex.captures(vid).unwrap();
        if let Some(vid_split) = url_regex.captures(vid) {
            vid = vid_split.get(1)
                    .expect("regex capture failure")
                    .as_str();
        }

        let url_info = format!("https://youtube.com/get_video_info?video_id={}", vid);

        let basic = {
            let mut url_response = send_request(&url_info)
                .chain_err(|| "Network request failed")?;

            let mut url_response_str = String::new();
            url_response.read_to_string(&mut url_response_str)
                .chain_err(|| "Respone from YT contained invalid UTF8")?;

            parse_url(&url_response_str)?
        };
        if basic["status"] != "ok" {
            bail!("Video not found");
        }

        let videoid      = &basic.get("video_id"      ).chain_err(|| "Error getting video_id from url")?;
        let length       = &basic.get("length_seconds").chain_err(|| "Error getting length_seconds from url")?;

        let (streams, videostreams, audiostreams) = Self::get_streams(&basic)?;

        Ok(Self {
            id: videoid.to_string(),
            length: length.parse::<u32>().unwrap(),
            streams,
            videostreams,
            audiostreams,
        })
    }

    fn get_streams(basic: &HashMap<String, String>) -> Result<(Vec<Stream>, Vec<Stream>, Vec<Stream>)> {
        let mut parsed_streams: Vec<Stream> = Vec::new();
        let streams: Vec<&str> = basic["url_encoded_fmt_stream_map"]
            .split(',')
            .collect();

        for url in streams.iter() {
            let parsed = parse_url(url)?;
            let extension = &parsed["type"]
                .split('/')
                .nth(1)
                .unwrap()
                .split(';')
                .next()
                .unwrap();
            let quality = &parsed["quality"];
            let stream_url = &parsed["url"];

            let parsed_stream = Stream {
                        extension: extension.to_string(),
                        quality: quality.to_string(),
                        url: stream_url.to_string(),
                    };

            parsed_streams.push(parsed_stream);
        }

        let mut parsed_videostreams: Vec<Stream> = Vec::new();
        let mut parsed_audiostreams: Vec<Stream> = Vec::new();

        if basic.contains_key("adaptive_fmts") {
            let streams: Vec<&str> = basic["adaptive_fmts"]
                .split(',')
                .collect();

            for url in streams.iter() {
                let parsed = parse_url(url)?;
                let extension = &parsed["type"]
                    .split('/')
                    .nth(1)
                    .unwrap()
                    .split(';')
                    .next()
                    .unwrap();
                let stream_url = &parsed["url"];

                if parsed.contains_key("quality_label") {
                    let quality = &parsed["quality_label"];
                    let parsed_videostream = Stream {
                        extension: extension.to_string(),
                        quality: quality.to_string(),
                        url: stream_url.to_string(),
                    };

                    parsed_videostreams.push(parsed_videostream);
                } else {
                    let audio_extension = if extension == &"mp4" {"m4a"} else {extension};
                    let quality = &parsed["bitrate"];
                    let parsed_audiostream = Stream {
                        extension: audio_extension.to_string(),
                        quality: quality.to_string(),
                        url: stream_url.to_string(),
                    };

                    parsed_audiostreams.push(parsed_audiostream);
                }
            }
        }

        Ok((parsed_streams, parsed_videostreams, parsed_audiostreams))
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


fn parse_url(query: &str) -> Result<HashMap<String, String>> {
    let url = format!("{}{}", "http://e.com?", query);
    let parsed_url = hyper::Url::parse(&url).chain_err(|| format!("Error parsing url {}", url))?;
    Ok(parsed_url.query_pairs().into_owned().collect())
}

// get file size from Content-Length header
fn get_file_size(response: &Response) -> Result<u64> {
    response.headers
        .get::<ContentLength>()
        .map(|length| length.0)
        .chain_err(|| "Content-Length header missing")
}
fn send_request(url: &str) -> Result<Response> {
    let ssl = NativeTlsClient::new().chain_err(|| "Error building NativeTlsClient")?;
    let connector = HttpsConnector::new(ssl);
    let client = Client::with_connector(connector);
    // Pass custom headers to fix speed throttle (issue #10)
    let mut header = Headers::new();
    header.set(Range::Bytes(vec![ByteRangeSpec::AllFrom(0)]));
    client.get(url).headers(header).send().chain_err(|| format!("Error sending request to {}", url))
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

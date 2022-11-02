use youtube_dl::{YoutubeDl, YoutubeDlOutput};

use log::warn;

use super::UrlContentProvider;

pub struct YtDlContentProvider {
    urls: Vec<String>,
}

impl YtDlContentProvider {
    pub fn new(url: &str) -> Option<YtDlContentProvider> {
        let info = YoutubeDl::new(url).run().unwrap();

        match info {
            YoutubeDlOutput::SingleVideo(video) => {
                if let Some(formats) = video.formats {
                    let urls = formats
                        .into_iter()
                        .filter(|f| f.acodec.is_some() && f.vcodec.is_none() && f.url.is_some())
                        .map(|f| f.url)
                        .flatten()
                        .collect::<Vec<_>>();
                    println!("Found urls: {:?}", urls);
                    Some(YtDlContentProvider { urls })
                } else {
                    None
                }
            }
            YoutubeDlOutput::Playlist(_playlist) => {
                warn!("Playlists are currently not supported");
                None
            }
        }
    }
}

impl UrlContentProvider for YtDlContentProvider {
    fn urls(&self) -> Vec<&str> {
        self.urls.iter().map(String::as_str).collect()
    }
}

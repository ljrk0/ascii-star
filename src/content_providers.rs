//! Abstracts over content providers for the songs.

mod youtube_dl;

use self::youtube_dl::YtDlContentProvider;

/// A content provider that uses URLs to provide content.
pub trait UrlContentProvider {
    /// Returns a list of usable URLs.
    fn urls(&self) -> Vec<&str>;
}

/// Returns the fitting content provider for the given path.
pub fn get_url_content_provider(url: &str) -> Box<dyn UrlContentProvider> {
    if url.starts_with("file://") {
        Box::new(SimpleURLProvider::from_url(url))
    } else if url.starts_with("http://") || url.starts_with("https://") {
        if url.contains("youtu.be") || url.contains("youtube") {
            Box::new(YtDlContentProvider::new(url).unwrap())
        } else {
            Box::new(SimpleURLProvider::from_url(url))
        }
    } else {
        Box::new(SimpleURLProvider::from_local_path(url))
    }
}

/// Represents a local file to be played.
struct SimpleURLProvider {
    /// The path of the file.
    url: String
}

impl SimpleURLProvider {
    /// Create a new simple url provider for a specific url.
    fn from_url(url: &str) -> SimpleURLProvider {
        SimpleURLProvider {
            url: url.to_string()
        }
    }

    /// Create a new simple url provider for a local file.
    fn from_local_path(path: &str) -> SimpleURLProvider {
        let mut url = String::from("file://");
        url.push_str(path);

        SimpleURLProvider {
            url
        }
    }
}

impl UrlContentProvider for SimpleURLProvider {
    fn urls(&self) -> Vec<&str> {
        vec![&self.url]
    }
}

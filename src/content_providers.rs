//! Abstracts over content providers for the songs.

mod youtube;

pub trait UrlContentProvider {
    fn urls(&self) -> Vec<&str>;
}

/// Abstracts over a content provider for the songs.
pub trait ContentProvider {
    /// Returns the path to a local file, if applicable.
    fn get_local_file_path(&self) -> Option<&str>;
}

/// Returns the fitting content provider for the given path.
pub fn get_content_provider(path: &str) -> Box<ContentProvider> {
    // This would be where the type of content provider is checked.
    // Currently there is only the local file provider
    Box::new(LocalFileProvider::from_audio_path(path))
}

/// Represents a local file to be played.
struct LocalFileProvider {
    /// The path of the file.
    uri: String
}

impl LocalFileProvider {
    /// Create a new local file provider for a specific file.
    fn from_audio_path(path: &str) -> LocalFileProvider {
        let mut uri = String::from("file://");
        uri.push_str(path);

        LocalFileProvider {
            uri
        }
    }
}

impl ContentProvider for LocalFileProvider {
    fn get_local_file_path(&self) -> Option<&str> {
        Some(&self.uri)
    }
}

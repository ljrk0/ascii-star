# ascii-star

A command line player for Ultrastar songs. An example cc licensed song is provided in the repo.

## Dependencies

The code currently uses the following system libraries not bundled within the
Rust application:

* gstreamer
* gst-plugins

## Usage

To play a local file specified by the `song.txt` metadata, run
```
$ cargo build
$ cargo run -- --local <ultrastar txt>
```

To use the remote song server, run
```
$ cargo run -- --search "<keywords>"
```

## Content Providers

We extended the `song.txt` metadata file format to contain not only
```
#MP3:Songfile.mp3
```
but also URIs such as `file://` and `http://` or `https://`:
```
#MP3:file://Songfile.mp3
#MP3:https://youtube.com/...
```
Depending on the URL given, different content providers trigger.  Perhaps this
should later be changed to
```
#MP3:spotify://...
#MP3:youtube://
```
to not rely on heuristics for the content provider and use `http[s]` heuristics
only as fallback.

In general, content providers implement the trait `UrlContentProvider` defined
in `src/content_providers.rs`.  Eg. the YouTube content provider
`src/content_providers/youtube.rs` defines a `struct YouTube` with a
constructor `::new(<youtube-url>)` which returns such an object.  The actual
trait consists simply of a a function `urls()` that returns a vector of
playable resource URLs.

### YouTube Content Provider

Unfortunately this content provider is currently out of date as it uses a
legacy API.  The `get_video_info` API call doesn't immediately expose the
`video_id` and `length_seconds` but encodes them in a JSON object called
`player_response` which in turn contains

* a `videoDetails` object containing `videoId` and `lengthSeconds` with the same
  function as before
* a `streamingData` object containing arrays `formats` and `adaptive_formats`
  with streaming URLs embedded.

However, not all videos use this API, especially older or music videos seem to
be hard to access.  It's probably better to use the crate
[youtube-dl](https://docs.rs/youtube_dl/0.5.0/youtube_dl/) for the heavy lifting.

## Remote Song Server

The project https://github.com/aticu/ascii-star-server/ implements a remote
video lookup server which by default is queried under the `SERVER_URL`
of `http://localhost:8080`.  It can serve music files as well.

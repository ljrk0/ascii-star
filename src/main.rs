#![recursion_limit = "1024"]
#[macro_use]
extern crate error_chain;

extern crate alto;
extern crate clap;
extern crate colored;
extern crate env_logger;
extern crate gstreamer as gst;
#[macro_use]
extern crate log;
extern crate pitch_calc;
extern crate termion;
extern crate ultrastar_txt;
// extern crate hyper;
// extern crate hyper_native_tls;
extern crate regex;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

extern crate glib;

mod content_providers;
mod draw;
mod pitch;
mod server_interface;

use crate::content_providers::get_url_content_provider;

use std::io::{stdout, Write};
use std::path::PathBuf;
use crate::gst::MessageView;
use crate::gst::prelude::*;
use clap::{App, Arg, ArgGroup};
use termion::screen::AlternateScreen;
use alto::{Alto, Capture, Mono};
use std::thread;
use std::sync::{Arc, Mutex};
use pitch_calc::*;

mod errors {
    error_chain!{}
}
use crate::errors::*;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const AUTHOR: &'static str = env!("CARGO_PKG_AUTHORS");

struct CustomData {
    playbin: gst::Element,    // Our one and only element
    playing: bool,            // Are we in the PLAYING state?
    terminate: bool,          // Should we terminate execution?
    duration: gst::ClockTime, // How long does this media last, in nanoseconds
}

fn main() {
    if let Err(ref e) = run() {
        use std::io::Write;
        let stderr = &mut ::std::io::stderr();
        let errmsg = "Error writing to stderr";

        writeln!(stderr, "error: {}", e).expect(errmsg);

        for e in e.iter().skip(1) {
            writeln!(stderr, "caused by: {}", e).expect(errmsg);
        }

        // The backtrace is not always generated. Try to run this example
        // with `RUST_BACKTRACE=1`.
        if let Some(backtrace) = e.backtrace() {
            writeln!(stderr, "backtrace: {:?}", backtrace).expect(errmsg);
        }

        ::std::process::exit(1);
    }
}

const SAMPLE_RATE: u32 = 44_100;
const FRAMES: i32 = 2048;

fn run() -> Result<()> {
    let _ = env_logger::init();

    // manage command line arguments using clap
    let matches = App::new("usrs-cli")
        .version(VERSION)
        .author(AUTHOR)
        .about("An Ultrastar song player for the command line written in rust")
        // xor: either local or search, but not both
        .group(ArgGroup::with_name("content_providers").args(&["local", "search"]).required(true))
        .args(&[
            Arg::with_name("local")
                .value_name("TXT")
                .short("l")
                .long("local")
                .help("the song file to play"),
            Arg::with_name("search")
                .value_name("KEYWORD")
                .short("s")
                .long("search")
                .help("a keyword to search on the server"),
            Arg::with_name("play")
                .requires("search") //<
                .value_name("INDEX")
                .short("p")
                .long("play")
                .help("index from search list to play")
                // TODO: add validation (value should be an int!)
        ])
        .get_matches();

    println!("Ultrastar CLI player {} by @man0lis", VERSION);

    let tempfile = if let Some(keyword) = matches.value_of("search") {
        // did we get a `play` argument as well?
        if let Some(index) = matches.value_of("play") {
            let index = index.parse::<usize>().chain_err(|| "index has to be an integer")?;
            let url = server_interface::search(keyword, Some(index))?.unwrap();

            Some(server_interface::download_file(url)
                .chain_err(|| "could not download .txt file")?)
        } else {
            // this is an exit point!
            server_interface::search(keyword, None)?;
            return Ok(());
        }
    } else {
        None
    };

    // TODO: download text file into /tmp
    // TODO: maybe use crate `tempfile` for this?
    // TODO: pass this tmp path to ultrastar_txt

    // get path from tempfile or command line arguments.
    // unwrap should not fail because tempfile is none => no `search` was done => `local` argument is required
    let song_filepath: PathBuf = match &tempfile {
        Some(file) => PathBuf::from(file.path()),
        None => PathBuf::from(matches.value_of("local").unwrap())
    };

    // parse txt file
    let txt_song = ultrastar_txt::parse_txt_song(song_filepath)
        .chain_err(|| "could not parse song file")?;
    let header = txt_song.header;
    let lines = txt_song.lines;

    // prepare song
    let bpms = header.bpm / 60.0 / 1000.0;
    let gap = header.gap.unwrap_or(0.0);

    let mut line_iter = lines.into_iter();
    let mut current_line = line_iter.next();
    let mut next_line = line_iter.next();

    // construct path and uri to audio file
    let audio_path = header.audio_path;
    let content_provider = get_url_content_provider(&audio_path);

    // set up openal for capture
    let alto = Alto::load_default().chain_err(|| "could not load openal default implementation")?;
    let cap_dev = alto.default_capture().unwrap();
    let mut capture: Capture<Mono<i16>> = alto.open_capture(Some(&cap_dev), SAMPLE_RATE, FRAMES)
        .chain_err(|| "could not open default capture device")?;

    // reference counted mutex for current deteced note
    let detected_note = Arc::new(Mutex::new(Some(LetterOctave(Letter::C, 2))));
    let detected_note_capture = detected_note.clone();

    // thread that handels audio buffers from openal the audio buffer
    let capture_thread = move || {
        capture.start();
        loop {
            let mut samples_len = capture.samples_len();
            let mut buffer_i16: Vec<i16> = vec![0; FRAMES as usize];
            while samples_len < buffer_i16.len() as i32 {
                samples_len = capture.samples_len();
                thread::sleep(std::time::Duration::from_millis(1));
            }
            capture
                .capture_samples(&mut buffer_i16)
                .chain_err(|| "could not capture samples")
                .unwrap();
            let buffer_f32: Vec<_> = buffer_i16
                .iter()
                .map(|x| (*x as f32) / (std::i16::MAX as f32) * 2.0)
                .collect();
            let max_volume = pitch::get_max_amplitude(buffer_f32.as_ref());
            let mut dominant_note = detected_note_capture.lock().unwrap();
            *dominant_note = if max_volume > 0.1 {
                Some(pitch::get_dominant_note(
                    buffer_f32.as_ref(),
                    SAMPLE_RATE as f64,
                ))
            } else {
                None
            };
        }
    };

    // initialize GStreamer
    gst::init().unwrap();

    // create the playbin element
    let playbin = gst::ElementFactory::make("playbin", Some("playbin"))
        .chain_err(|| "failed to create playbin element")?;

    // set the URI to play
    for url in content_provider.urls() {
        playbin
            .set_property("uri", &url)
            .chain_err(|| "can't set uri property on playbin")?;

        break
    }

    // disable video and subtitle, if they exist
    // according to: https://github.com/sdroege/gstreamer-rs/blob/4117c01ff2c9ce9b46b8f63315af4dc284788e9b/examples/src/bin/playbin.rs#L27-L35
    let flags = playbin
        .get_property("flags")
        .chain_err(|| "can't get playbin flags")?;
    let flags_class = ::glib::FlagsClass::new(flags.type_()).unwrap();
    let flags = flags_class.builder_with_value(flags).unwrap()
        .unset_by_nick("text")
        .unset_by_nick("video")
        .build()
        .unwrap();
    playbin
        .set_property("flags", &flags)
        .chain_err(|| "can't set playbin flags")?;

    println!("Playing {} by {}...\n", header.title, header.artist);

    // Start playing
    let ret = playbin.set_state(gst::State::Playing);
    assert_ne!(ret.is_err(), true);

    // connect to the bus
    let bus = playbin.get_bus().unwrap();
    let mut custom_data = CustomData {
        playbin: playbin,
        playing: false,
        terminate: false,
        duration: gst::CLOCK_TIME_NONE,
    };

    thread::spawn(capture_thread);

    // get access to terminal
    //let stdin = stdin();
    //let mut stdout = stdout();
    let mut stdout = AlternateScreen::from(stdout());

    // clear screen
    write!(stdout, "{}", termion::clear::All).chain_err(|| "could not write to stdout")?;

    // begin main loop
    while !custom_data.terminate {
        let msg = bus.timed_pop(10 * gst::MSECOND);

        match msg {
            Some(msg) => {
                handle_message(&mut custom_data, &msg);
            }
            None => {
                if custom_data.playing {
                    let position = custom_data
                        .playbin
                        .query_position()
                        .unwrap_or(gst::CLOCK_TIME_NONE);

                    // If we didn't know it yet, query the stream duration
                    if custom_data.duration == gst::CLOCK_TIME_NONE {
                        custom_data.duration = custom_data
                            .playbin
                            .query_duration()
                            .unwrap_or(gst::CLOCK_TIME_NONE);
                    }
                    // get note from capture thread
                    let dominant_note = detected_note.lock().unwrap().clone();
                    // calculate current beat
                    let position_ms = position.mseconds().unwrap_or(0) as f32;
                    // don't know why I need the 4.0 but its in the
                    // original game and its not working without it
                    let beat = (position_ms - gap) * (bpms * 4.0);

                    let next_line_start = if next_line.is_some() {
                        next_line.clone().unwrap().start
                    } else {
                        // last line reached, make next if always fail
                        beat as i32 + 100
                    };
                    if beat > next_line_start as f32 {
                        // reprint current line to avoid stale highlights
                        if let &Some(ref line) = &current_line {
                            write!(
                                stdout,
                                "{}",
                                draw::generate_screen(line, beat + 100.0, dominant_note)?
                            ).chain_err(|| "could not write to stdout")?;
                        }

                        if next_line.is_some() {
                            current_line = next_line;
                        };
                        next_line = line_iter.next();
                        // clear screen
                        write!(stdout, "{}", termion::clear::All)
                            .chain_err(|| "could not write to stdout")?;
                    }

                    // print current lyric line
                    if let &Some(ref line) = &current_line {
                        write!(
                            stdout,
                            "{}",
                            draw::generate_screen(line, beat, dominant_note)?
                        ).chain_err(|| "could not write to stdout")?;
                    }
                }
            }
        }
    }
    // end main loop

    // Shutdown pipeline
    let ret = custom_data.playbin.set_state(gst::State::Null);
    assert_ne!(ret.is_err(), true);

    println!("");
    Ok(())
}

fn handle_message(custom_data: &mut CustomData, msg: &gst::GstRc<gst::MessageRef>) {
    match msg.view() {
        MessageView::Error(err) => {
            error!(
                "Error received from element {:?}: {} ({:?})",
                msg.get_src().map(|s| s.get_path_string()),
                err.get_error(),
                err.get_debug()
            );
            custom_data.terminate = true;
        }
        MessageView::Eos(..) => {
            info!("End-Of-Stream reached.");
            custom_data.terminate = true;
        }
        MessageView::DurationChanged(_) => {
            // The duration has changed, mark the current one as invalid
            custom_data.duration = gst::CLOCK_TIME_NONE;
        }
        MessageView::StateChanged(state) => if msg.get_src()
            .map(|s| s == custom_data.playbin)
            .unwrap_or(false)
        {
            let new_state = state.get_current();
            let old_state = state.get_old();

            info!(
                "Pipeline state changed from {:?} to {:?}",
                old_state, new_state
            );

            custom_data.playing = new_state == gst::State::Playing;
        },
        _ => (),
    }
}

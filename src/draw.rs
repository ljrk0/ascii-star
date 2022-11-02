mod errors {
    use error_chain::error_chain;

    error_chain!{}
}
use crate::errors::*;

use colored::*;
use pitch_calc::*;

pub fn generate_screen(
    line: &ultrastar_txt::Line,
    beat: f32,
    dominant_note: Option<LetterOctave>,
) -> Result<String> {
    let (term_width, _term_height) =
        termion::terminal_size().chain_err(|| "could not get terminal size")?;
    let note_lines = draw_notelines(line, beat, term_width)?;
    let lyric_line = gen_lyric_line(line, beat, term_width, dominant_note);

    Ok(format!("{}{}", note_lines, lyric_line,))
}

fn draw_notelines(line: &ultrastar_txt::Line, beat: f32, term_width: u16) -> Result<String> {
    // spacin between note lines
    let line_spacing = 2;
    // space to leave at the top (ex for progrss bar)
    let top_offset = 2;

    let mut output = String::new();

    let first_note_start = if let Some(note) = line.notes.first() {
        match *note {
            ultrastar_txt::Note::Regular {
                start,
                duration: _,
                pitch: _,
                text: _,
            } => start,
            ultrastar_txt::Note::Golden {
                start,
                duration: _,
                pitch: _,
                text: _,
            } => start,
            ultrastar_txt::Note::Freestyle {
                start,
                duration: _,
                pitch: _,
                text: _,
            } => start,
            ultrastar_txt::Note::PlayerChange { player: _ } => 0, // TODO: this is bad find better solution
        }
    } else {
        return Err("line has no first note???".into());
    };

    let last_note_end = if let Some(note) = line.notes.last() {
        match *note {
            ultrastar_txt::Note::Regular {
                start,
                duration,
                pitch: _,
                text: _,
            } => start + duration,
            ultrastar_txt::Note::Golden {
                start,
                duration,
                pitch: _,
                text: _,
            } => start + duration,
            ultrastar_txt::Note::Freestyle {
                start,
                duration,
                pitch: _,
                text: _,
            } => start + duration,
            ultrastar_txt::Note::PlayerChange { player: _ } => 0, // TODO: this is bad find better solution
        }
    } else {
        return Err("line has no last note???".into());
    };

    let chars_per_beat = term_width as f32 / (last_note_end - first_note_start) as f32;

    for note in line.notes.iter() {
        let (start, duration, pitch, note_type) = match *note {
            ultrastar_txt::Note::Regular {
                start,
                duration,
                pitch,
                text: _,
            } => (start, duration, Step(pitch as f32), NoteType::Regular),
            ultrastar_txt::Note::Golden {
                start,
                duration,
                pitch,
                text: _,
            } => (start, duration, Step(pitch as f32), NoteType::Golden),
            ultrastar_txt::Note::Freestyle {
                start,
                duration,
                pitch,
                text: _,
            } => (start, duration, Step(pitch as f32), NoteType::Freestyle),
            _ => continue,
        };

        // calculate position of current note
        // terminal goto starts at 1
        let note_hpos = ((start - first_note_start) as f32 * chars_per_beat) as u16 + 1;
        let note_vpos =
            (top_offset + 17 * line_spacing) - letter_to_pos(pitch.letter()) * line_spacing + 1;

        let color_note = match note_type {
            NoteType::Golden => {
                Box::new(|note: &str| note.yellow().to_string()) as Box<dyn Fn(&str) -> String>
            }
            NoteType::Regular => {
                Box::new(|note: &str| note.bright_blue().to_string()) as Box<dyn Fn(&str) -> String>
            }
            NoteType::Freestyle => {
                Box::new(|note: &str| note.red().to_string()) as Box<dyn Fn(&str) -> String>
            }
        };

        let color_played_note = match note_type {
            NoteType::Golden => {
                Box::new(|note: &str| note.bright_yellow().to_string()) as Box<dyn Fn(&str) -> String>
            }
            NoteType::Regular => {
                Box::new(|note: &str| note.white().to_string()) as Box<dyn Fn(&str) -> String>
            }
            NoteType::Freestyle => {
                Box::new(|note: &str| note.bright_red().to_string()) as Box<dyn Fn(&str) -> String>
            }
        };

        // note is current note or allready played
        if beat >= start as f32 {
            // draw progress bar
            let times = (beat - start as f32) * chars_per_beat;
            if beat <= last_note_end as f32 {
                let bar = "#".repeat(times.floor() as usize);
                // terminal goto starts with 1
                output.push_str(format!("{}{}", termion::cursor::Goto(1, 1), bar).as_ref());
            }

            // note is current note -> hightlight it
            if (start + duration) as f32 >= beat {
                let marked = (beat - start as f32) * chars_per_beat;
                let note_line_str = color_note(
                    "#".repeat((duration as f32 * chars_per_beat) as usize)
                        .as_ref(),
                );
                let marked_line_str = color_played_note("#".repeat(marked as usize).as_ref());
                output.push_str(
                    format!(
                        "{}{}{}{}{}{:?}",
                        termion::cursor::Goto(note_hpos, note_vpos),
                        note_line_str,
                        termion::cursor::Goto(note_hpos, note_vpos),
                        marked_line_str,
                        termion::cursor::Goto(note_hpos, note_vpos),
                        pitch.letter(),
                    ).as_ref(),
                );
            }
            // note has been played
            else {
                let played_line_str = color_played_note(
                    "#".repeat((duration as f32 * chars_per_beat) as usize)
                        .as_ref(),
                );
                output.push_str(
                    format!(
                        "{}{}{}{:?}",
                        termion::cursor::Goto(note_hpos, note_vpos),
                        played_line_str,
                        termion::cursor::Goto(note_hpos, note_vpos),
                        pitch.letter(),
                    ).as_ref(),
                );
            }
        // note has not been played yet
        } else {
            let note_line_str = color_note(
                "#".repeat((duration as f32 * chars_per_beat) as usize)
                    .as_ref(),
            );
            output.push_str(
                format!(
                    "{}{}{}{:?}",
                    termion::cursor::Goto(note_hpos, note_vpos),
                    note_line_str,
                    termion::cursor::Goto(note_hpos, note_vpos),
                    pitch.letter(),
                ).as_ref(),
            );
        }
    }

    Ok(output)
}

fn line_to_str(line: &ultrastar_txt::Line) -> String {
    let mut line_str = String::new();
    for note in line.notes.iter() {
        match *note {
            ultrastar_txt::Note::Regular {
                start: _,
                duration: _,
                pitch: _,
                ref text,
            } => line_str.push_str(text),
            ultrastar_txt::Note::Golden {
                start: _,
                duration: _,
                pitch: _,
                ref text,
            } => line_str.push_str(text),
            ultrastar_txt::Note::Freestyle {
                start: _,
                duration: _,
                pitch: _,
                ref text,
            } => line_str.push_str(text),
            _ => continue,
        };
    }
    line_str
}

#[derive(PartialEq)]
enum NoteType {
    Regular,
    Golden,
    Freestyle,
}

fn gen_lyric_line(
    line: &ultrastar_txt::Line,
    beat: f32,
    term_width: u16,
    dominant_note: Option<LetterOctave>,
) -> String {
    let uncolored_line = line_to_str(line);

    // terminal goto starts at 1
    let line_vpos = (term_width - uncolored_line.len() as u16) / 2 + 1;
    let line_hpos = 2 + 17 * 2 + 10 + 1; // TODO this is below the lines but should not be a magic number

    let mut lyric = format!("{}", termion::cursor::Goto(line_vpos, line_hpos));
    for note in line.notes.iter() {
        let (start, duration, _pitch, text, note_type) = match *note {
            ultrastar_txt::Note::Regular {
                start,
                duration,
                pitch,
                ref text,
            } => (start, duration, pitch, text, NoteType::Regular),
            ultrastar_txt::Note::Golden {
                start,
                duration,
                pitch,
                ref text,
            } => (start, duration, pitch, text, NoteType::Golden),
            ultrastar_txt::Note::Freestyle {
                start,
                duration,
                pitch,
                ref text,
            } => (start, duration, pitch, text, NoteType::Freestyle),
            _ => continue,
        };

        // note is current note or allready played
        if beat >= start as f32 {
            // note is current note -> hightlight it
            if (start + duration) as f32 >= beat {
                if note_type == NoteType::Golden {
                    lyric.push_str(&text.black().on_bright_yellow().to_string());
                } else {
                    lyric.push_str(&text.black().on_bright_white().to_string());
                }
            }
            // note has been played
            else if note_type == NoteType::Golden {
                lyric.push_str(&text.yellow().to_string());
            } else {
                lyric.push_str(&text.white().to_string());
            }
        } else if note_type == NoteType::Golden {
            lyric.push_str(&text.bright_yellow().to_string());
        } else {
            lyric.push_str(&text.bright_blue().to_string());
        }
    }
    // add current note under the line
    let note = match dominant_note {
        Some(n) => format!("{:?}", n),
        None => "                    ".to_string(),
    };
    let line_hpos = 2 + 17 * 2 + 10 + 3; // TODO this is below the lines but should not be a magic number
    let line_vpos = (term_width - note.len() as u16) / 2 + 1;
    lyric.push_str(format!("{}{}", termion::cursor::Goto(line_vpos, line_hpos), note).as_ref());

    lyric
}

fn letter_to_pos(letter: Letter) -> u16 {
    match letter {
        Letter::C => 0,
        Letter::Csh => 1,
        Letter::Db => 2,
        Letter::D => 3,
        Letter::Dsh => 4,
        Letter::Eb => 5,
        Letter::E => 6,
        Letter::F => 7,
        Letter::Fsh => 8,
        Letter::Gb => 9,
        Letter::G => 10,
        Letter::Gsh => 11,
        Letter::Ab => 12,
        Letter::A => 13,
        Letter::Ash => 14,
        Letter::Bb => 15,
        Letter::B => 16,
    }
}

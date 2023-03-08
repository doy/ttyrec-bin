#![warn(clippy::cargo)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::as_conversions)]
#![warn(clippy::get_unwrap)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::similar_names)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::type_complexity)]

use clap::Parser as _;

mod display;
mod event;
mod frames;
mod input;
mod timer;

#[derive(Debug, clap::Parser)]
#[command(
    name = "ttyplay",
    about = "Plays back ttyrec files",
    long_about = "\n\
        This is a player for ttyrec files. It allows arbitrary seeking, both \
        forward and backward, as well as searching through the file for \
        output. Playback can be paused using the Space key, and the rest of \
        the key bindings can be found by pressing `?` while the player is \
        paused."
)]
struct Opt {
    #[arg(
        short,
        long,
        default_value = "ttyrec",
        help = "File to read ttyrec data from"
    )]
    file: std::ffi::OsString,

    #[arg(
        long,
        help = "Restrict time between frames to at most this many milliseconds"
    )]
    clamp: Option<u64>,

    #[arg(short, long, help = "Start the player paused")]
    paused: bool,

    #[arg(
        short,
        long,
        default_value = "4",
        help = "Speed to run the playback at. This can be a number from 0-8, \
            where higher is faster."
    )]
    speed: u32,
}

#[tokio::main]
async fn async_main(opt: Opt) -> anyhow::Result<()> {
    let Opt {
        file,
        clamp,
        paused,
        speed,
    } = opt;

    let speed = speed.clamp(0, 8);

    let fh = tokio::fs::File::open(file).await?;

    let mut input = textmode::blocking::Input::new()?;
    let mut output = textmode::Output::new().await?;
    let _input_guard = input.take_raw_guard();
    let _output_guard = output.take_screen_guard();

    let (event_w, event_r) = tokio::sync::mpsc::unbounded_channel();
    let (timer_w, timer_r) = tokio::sync::mpsc::unbounded_channel();

    input::spawn_thread(event_w.clone(), input);

    let frame_data = std::sync::Arc::new(tokio::sync::Mutex::new(
        frames::FrameData::new(),
    ));
    frames::load_from_file(frame_data.clone(), fh, event_w.clone(), clamp);

    let timer_task = timer::spawn_task(
        event_w.clone(),
        frame_data.clone(),
        timer_r,
        paused,
        speed,
    );

    event::handle_events(event_r, timer_w.clone(), output).await?;

    timer_w.send(event::TimerAction::Quit)?;
    timer_task.await?;

    Ok(())
}

fn main() {
    let opt = Opt::parse();
    match async_main(opt) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("ttyplay: {e}");
            std::process::exit(1);
        }
    };
}

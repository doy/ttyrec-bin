#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::struct_excessive_bools)]

mod display;
mod event;
mod frames;
mod input;
mod timer;

#[derive(Debug, structopt::StructOpt)]
#[structopt(about = "ttyplay")]
struct Opt {
    #[structopt(short, long, default_value = "ttyrec")]
    file: std::ffi::OsString,

    #[structopt(long)]
    clamp: Option<u64>,
}

async fn async_main(opt: Opt) -> anyhow::Result<()> {
    let Opt { file, clamp } = opt;

    let fh = async_std::fs::File::open(file).await?;

    let mut input = textmode::Input::new().await?;
    let mut output = textmode::Output::new().await?;
    let _input_guard = input.take_raw_guard();
    let _output_guard = output.take_screen_guard();

    let (event_w, event_r) = async_std::channel::unbounded();
    let (timer_w, timer_r) = async_std::channel::unbounded();

    input::spawn_task(event_w.clone(), input);

    let frame_data = async_std::sync::Arc::new(async_std::sync::Mutex::new(
        frames::FrameData::new(),
    ));
    frames::load_from_file(frame_data.clone(), fh, event_w.clone(), clamp);

    let timer_task =
        timer::spawn_task(event_w.clone(), frame_data.clone(), timer_r);

    event::handle_events(event_r, timer_w.clone(), output).await?;

    timer_w.send(event::TimerAction::Quit).await?;
    timer_task.await;

    Ok(())
}

#[paw::main]
fn main(opt: Opt) {
    match async_std::task::block_on(async_main(opt)) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("ttyplay: {}", e);
            std::process::exit(1);
        }
    };
}

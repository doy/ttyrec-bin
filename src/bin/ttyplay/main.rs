use async_std::prelude::FutureExt as _;

mod display;
mod event;
mod frames;
mod input;

#[derive(Debug, structopt::StructOpt)]
#[structopt(about = "ttyplay")]
struct Opt {
    #[structopt(short, long, default_value = "ttyrec")]
    file: std::ffi::OsString,
}

enum TimerAction {
    Pause,
    GotoFrame(usize),
    Quit,
}

fn spawn_frame_reader_task(
    event_w: async_std::channel::Sender<event::Event>,
    frames: async_std::sync::Arc<async_std::sync::Mutex<frames::FrameData>>,
    fh: async_std::fs::File,
) {
    async_std::task::spawn(async move {
        let mut reader = ttyrec::Reader::new(fh);
        let size = terminal_size::terminal_size().map_or(
            (24, 80),
            |(terminal_size::Width(w), terminal_size::Height(h))| (h, w),
        );
        let mut parser = vt100::Parser::new(size.0, size.1, 0);
        while let Ok(frame) = reader.read_frame().await {
            let delay = if let Some(time) = reader.offset() {
                frame.time - time
            } else {
                std::time::Duration::from_secs(0)
            };
            parser.process(&frame.data);
            let mut frames = frames.lock_arc().await;
            frames
                .add_frame(frames::Frame::new(parser.screen().clone(), delay))
                .await;
            event_w
                .send(event::Event::FrameLoaded(Some(frames.count())))
                .await
                .unwrap();
        }
        frames.lock_arc().await.done_reading().await;
        event_w.send(event::Event::FrameLoaded(None)).await.unwrap();
    });
}

fn spawn_timer_task(
    event_w: async_std::channel::Sender<event::Event>,
    frames: async_std::sync::Arc<async_std::sync::Mutex<frames::FrameData>>,
    timer_r: async_std::channel::Receiver<TimerAction>,
) -> async_std::task::JoinHandle<()> {
    async_std::task::spawn(async move {
        let mut idx = 0;
        let mut start_time = std::time::Instant::now();
        let mut paused_time = None;
        let mut force_update_time = false;
        loop {
            enum Res {
                Wait(Option<vt100::Screen>),
                TimerAction(
                    Result<TimerAction, async_std::channel::RecvError>,
                ),
            }
            let wait = async {
                let wait_read = frames.lock_arc().await.wait_for_frame(idx);
                if wait_read.await {
                    let frame =
                        frames.lock_arc().await.get(idx).unwrap().clone();
                    if force_update_time {
                        let now = std::time::Instant::now();
                        start_time = now - frame.delay();
                        if paused_time.take().is_some() {
                            paused_time = Some(now)
                        }
                        force_update_time = false;
                    } else if paused_time.is_some() {
                        std::future::pending::<()>().await;
                    } else {
                        async_std::task::sleep(
                            (start_time + frame.delay())
                                .saturating_duration_since(
                                    std::time::Instant::now(),
                                ),
                        )
                        .await;
                    }
                    Res::Wait(Some(frame.into_screen()))
                } else {
                    Res::Wait(None)
                }
            };
            let action = async { Res::TimerAction(timer_r.recv().await) };
            match wait.race(action).await {
                Res::Wait(Some(screen)) => {
                    event_w
                        .send(event::Event::Render((idx, screen)))
                        .await
                        .unwrap();
                    idx += 1;
                }
                Res::Wait(None) => {
                    event_w.send(event::Event::Pause).await.unwrap();
                }
                Res::TimerAction(Ok(action)) => match action {
                    TimerAction::Pause => {
                        let now = std::time::Instant::now();
                        if let Some(time) = paused_time.take() {
                            start_time += now - time;
                        } else {
                            paused_time = Some(now);
                        }
                        event_w
                            .send(event::Event::Paused(paused_time.is_some()))
                            .await
                            .unwrap();
                    }
                    TimerAction::GotoFrame(new_idx) => {
                        idx = new_idx;
                        force_update_time = true;
                    }
                    TimerAction::Quit => break,
                },
                Res::TimerAction(Err(e)) => panic!("{}", e),
            }
        }
    })
}

fn spawn_input_task(
    event_w: async_std::channel::Sender<event::Event>,
    mut input: textmode::Input,
) {
    async_std::task::spawn(async move {
        while let Some(key) = input.read_key().await.unwrap() {
            event_w.send(event::Event::Key(key)).await.unwrap();
        }
    });
}

async fn async_main(opt: Opt) -> anyhow::Result<()> {
    let Opt { file } = opt;

    let fh = async_std::fs::File::open(file).await?;

    let mut input = textmode::Input::new().await?;
    let mut output = textmode::Output::new().await?;
    let _input_guard = input.take_raw_guard();
    let _output_guard = output.take_screen_guard();

    let frames = async_std::sync::Arc::new(async_std::sync::Mutex::new(
        frames::FrameData::new(),
    ));
    let (event_w, event_r) = async_std::channel::unbounded();
    let (timer_w, timer_r) = async_std::channel::unbounded();

    spawn_frame_reader_task(event_w.clone(), frames.clone(), fh);
    spawn_input_task(event_w.clone(), input);
    let timer_task =
        spawn_timer_task(event_w.clone(), frames.clone(), timer_r);

    let mut display = display::Display::new();
    let mut current_screen = vt100::Parser::default().screen().clone();
    loop {
        let event = event_r.recv().await?;
        match event {
            event::Event::Render((idx, screen)) => {
                current_screen = screen.clone();
                display.current_frame(idx);
                display.render(&screen, &mut output).await?;
            }
            event::Event::Key(key) => {
                input::handle_input(key, event_w.clone()).await?
            }
            event::Event::FrameLoaded(n) => {
                if let Some(n) = n {
                    display.total_frames(n);
                } else {
                    display.done_loading();
                }
                display.render(&current_screen, &mut output).await?;
            }
            event::Event::Pause => timer_w.send(TimerAction::Pause).await?,
            event::Event::Paused(paused) => {
                display.paused(paused);
                display.render(&current_screen, &mut output).await?;
            }
            event::Event::FirstFrame => {
                timer_w.send(TimerAction::GotoFrame(0)).await?;
            }
            event::Event::LastFrame => {
                timer_w
                    .send(TimerAction::GotoFrame(
                        display.get_total_frames() - 1,
                    ))
                    .await?;
            }
            event::Event::NextFrame => {
                timer_w
                    .send(TimerAction::GotoFrame(
                        display.get_current_frame() + 1,
                    ))
                    .await?;
            }
            event::Event::PreviousFrame => {
                timer_w
                    .send(TimerAction::GotoFrame(
                        display.get_current_frame() - 1,
                    ))
                    .await?;
            }
            event::Event::ToggleUi => {
                display.toggle_ui();
                display.render(&current_screen, &mut output).await?;
            }
            event::Event::Quit => {
                timer_w.send(TimerAction::Quit).await?;
                break;
            }
        }
    }

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

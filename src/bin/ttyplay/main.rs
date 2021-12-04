use textmode::Textmode as _;

#[derive(Debug, structopt::StructOpt)]
#[structopt(about = "ttyplay")]
struct Opt {
    #[structopt(short, long, default_value = "ttyrec")]
    file: std::ffi::OsString,
}

#[derive(Debug, Clone)]
struct Frame {
    screen: vt100::Screen,
    delay: std::time::Duration,
}

struct FrameData {
    frames: Vec<Frame>,
    done_reading: bool,
    new_frame_w: async_std::channel::Sender<Option<usize>>,
    new_frame_r: async_std::channel::Receiver<Option<usize>>,
}

impl FrameData {
    fn new() -> Self {
        let (new_frame_w, new_frame_r) = async_std::channel::unbounded();
        Self {
            frames: vec![],
            done_reading: false,
            new_frame_w,
            new_frame_r,
        }
    }

    fn get(&self, i: usize) -> Option<&Frame> {
        self.frames.get(i)
    }

    async fn add_frame(&mut self, frame: Frame) {
        self.frames.push(frame);
        self.new_frame_w
            .send(Some(self.frames.len()))
            .await
            .unwrap()
    }

    async fn done_reading(&mut self) {
        self.done_reading = true;
        self.new_frame_w.send(None).await.unwrap();
    }

    fn wait_for_frame(
        &self,
        i: usize,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = bool> + 'static + Send>,
    > {
        if i < self.frames.len() {
            return Box::pin(std::future::ready(true));
        }
        let new_frame_r = self.new_frame_r.clone();
        Box::pin(async move {
            while let Some(new_len) = new_frame_r.recv().await.unwrap() {
                if i < new_len {
                    return true;
                }
            }
            false
        })
    }
}

enum Event {
    Render { screen: vt100::Screen },
    Quit,
}

enum TimerAction {
    Pause,
    NewFrameRead,
}

fn spawn_frame_reader_task(
    frames: async_std::sync::Arc<async_std::sync::Mutex<FrameData>>,
    size: (u16, u16),
    fh: async_std::fs::File,
) {
    async_std::task::spawn(async move {
        let mut reader = ttyrec::Reader::new(fh);
        let mut parser = vt100::Parser::new(size.0, size.1, 0);
        while let Ok(frame) = reader.read_frame().await {
            let delay = if let Some(time) = reader.offset() {
                frame.time - time
            } else {
                std::time::Duration::from_secs(0)
            };
            parser.process(&frame.data);
            frames
                .lock_arc()
                .await
                .add_frame(Frame {
                    screen: parser.screen().clone(),
                    delay,
                })
                .await;
        }
        frames.lock_arc().await.done_reading().await;
    });
}

fn spawn_timer_task(
    frames: async_std::sync::Arc<async_std::sync::Mutex<FrameData>>,
    timer_r: async_std::channel::Receiver<TimerAction>,
    event_w: async_std::channel::Sender<Event>,
) {
    async_std::task::spawn(async move {
        let mut idx = 0;
        let start_time = std::time::Instant::now();
        loop {
            let wait = frames.lock_arc().await.wait_for_frame(idx);
            if !wait.await {
                break;
            }
            let frame = frames.lock_arc().await.get(idx).unwrap().clone();
            async_std::task::sleep(
                (start_time + frame.delay)
                    .saturating_duration_since(std::time::Instant::now()),
            )
            .await;
            event_w
                .send(Event::Render {
                    screen: frame.screen,
                })
                .await
                .unwrap();
            idx += 1;
        }
        event_w.send(Event::Quit).await.unwrap();
    });
}

async fn async_main(opt: Opt) -> anyhow::Result<()> {
    let Opt { file } = opt;

    let fh = async_std::fs::File::open(file).await?;
    let size = terminal_size::terminal_size().map_or(
        (24, 80),
        |(terminal_size::Width(w), terminal_size::Height(h))| (h, w),
    );

    let mut input = textmode::Input::new().await?;
    let mut output = textmode::Output::new().await?;
    let _input_guard = input.take_raw_guard();
    let _output_guard = output.take_screen_guard();

    let frames = async_std::sync::Arc::new(async_std::sync::Mutex::new(
        FrameData::new(),
    ));
    let (event_w, event_r) = async_std::channel::unbounded();
    let (timer_w, timer_r) = async_std::channel::unbounded();

    spawn_frame_reader_task(frames.clone(), size, fh);
    spawn_timer_task(frames.clone(), timer_r, event_w.clone());

    loop {
        let event = event_r.recv().await?;
        match event {
            Event::Render { screen } => {
                output.clear();
                output.move_to(0, 0);
                output.write(&screen.contents_formatted());
                output.refresh().await?;
            }
            Event::Quit => break,
        }
    }

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

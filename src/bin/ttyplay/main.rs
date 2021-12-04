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

async fn async_main(opt: Opt) -> anyhow::Result<()> {
    let Opt { file } = opt;

    let fh = async_std::fs::File::open(file).await?;
    let mut reader = ttyrec::Reader::new(fh);
    let size = terminal_size::terminal_size().map_or(
        (24, 80),
        |(terminal_size::Width(w), terminal_size::Height(h))| (h, w),
    );

    let mut input = textmode::Input::new().await?;
    let mut output = textmode::Output::new().await?;
    let _input_guard = input.take_raw_guard();
    let _output_guard = output.take_screen_guard();

    let frames =
        async_std::sync::Arc::new(async_std::sync::Mutex::new(vec![]));
    let (frame_count_w, frame_count_r) = async_std::channel::unbounded();

    {
        let frames = frames.clone();
        async_std::task::spawn(async move {
            let mut parser = vt100::Parser::new(size.0, size.1, 0);
            while let Ok(frame) = reader.read_frame().await {
                let delay = if let Some(time) = reader.offset() {
                    frame.time - time
                } else {
                    std::time::Duration::from_secs(0)
                };
                parser.process(&frame.data);
                let mut frames = frames.lock_arc().await;
                frames.push(Frame {
                    screen: parser.screen().clone(),
                    delay,
                });
                frame_count_w.send(Some(frames.len())).await.unwrap();
            }
            frame_count_w.send(None).await.unwrap();
        });
    }

    let start_time = std::time::Instant::now();
    let mut idx = 0;
    let mut prev_frame: Option<Frame> = None;
    loop {
        frame_count_r.recv().await?;
        let frame = if let Some(frame) = frames.lock_arc().await.get(idx) {
            frame.clone()
        } else {
            break;
        };
        if let Some(prev_frame) = prev_frame {
            let dur = (start_time + frame.delay)
                .saturating_duration_since(std::time::Instant::now());
            async_std::task::sleep(dur).await;
            output.write(&frame.screen.contents_diff(&prev_frame.screen));
        } else {
            output.write(&frame.screen.contents_formatted());
        }
        output.refresh().await?;
        idx += 1;
        prev_frame = Some(frame);
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

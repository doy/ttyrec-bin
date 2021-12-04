use textmode::Textmode as _;

#[derive(Debug, structopt::StructOpt)]
#[structopt(about = "ttyplay")]
struct Opt {
    #[structopt(short, long, default_value = "ttyrec")]
    file: std::ffi::OsString,
}

async fn async_main(opt: Opt) -> anyhow::Result<()> {
    let Opt { file } = opt;

    let fh = async_std::fs::File::open(file).await?;
    let mut reader = ttyrec::Reader::new(fh);

    let mut input = textmode::Input::new().await?;
    let mut output = textmode::Output::new().await?;
    let _input_guard = input.take_raw_guard();
    let _output_guard = output.take_screen_guard();

    let mut last_frame_time = None;
    while let Ok(frame) = reader.read_frame().await {
        if let Some(time) = last_frame_time {
            async_std::task::sleep(frame.time - time).await;
        }
        output.write(&frame.data);
        output.refresh().await?;
        last_frame_time = Some(frame.time);
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

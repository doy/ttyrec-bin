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

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

#[derive(Debug, structopt::StructOpt)]
#[structopt(
    name = "ttyrec",
    about = "Records ttyrec files",
    long_about = "\n\
        This program will run a shell (or other program specified by the -c \
        option), and record the full output, including timing information, \
        for later playback (such as via the included `ttyplay` command)."
)]
struct Opt {
    #[structopt(
        short,
        long,
        default_value = "ttyrec",
        help = "File to save ttyrec data to"
    )]
    file: std::ffi::OsString,

    #[structopt(short, long, help = "Command to run [default: $SHELL]")]
    cmd: Option<std::ffi::OsString>,
}

fn get_cmd(
    cmd: Option<std::ffi::OsString>,
) -> (std::ffi::OsString, Vec<std::ffi::OsString>) {
    cmd.map_or_else(
        || {
            let shell =
                std::env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
            (shell, vec![])
        },
        |cmd| {
            let mut exec_cmd = std::ffi::OsString::from("exec ");
            exec_cmd.push(cmd);
            ("/bin/sh".into(), vec!["-c".into(), exec_cmd])
        },
    )
}

#[derive(Debug)]
enum Event {
    Key(textmode::Result<Option<textmode::Key>>),
    Stdout(std::io::Result<Vec<u8>>),
    Resize((u16, u16)),
    Error(anyhow::Error),
    Quit,
}

#[tokio::main]
async fn async_main(opt: Opt) -> anyhow::Result<()> {
    let Opt { cmd, file } = opt;
    let (cmd, args) = get_cmd(cmd);

    let fh = tokio::fs::File::create(file).await?;

    let mut input = textmode::blocking::Input::new()?;
    let _input_guard = input.take_raw_guard();
    let mut stdout = tokio::io::stdout();

    let size = terminal_size::terminal_size().map_or(
        (24, 80),
        |(terminal_size::Width(w), terminal_size::Height(h))| (h, w),
    );
    let mut pty = pty_process::Pty::new()?;
    pty.resize(pty_process::Size::new(size.0, size.1))?;
    let pts = pty.pts()?;
    let mut child = pty_process::Command::new(cmd).args(args).spawn(&pts)?;

    let (event_w, mut event_r) = tokio::sync::mpsc::unbounded_channel();
    let (input_w, mut input_r) = tokio::sync::mpsc::unbounded_channel();
    let (resize_w, mut resize_r) = tokio::sync::mpsc::unbounded_channel();

    {
        let mut signals = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::window_change(),
        )?;
        let event_w = event_w.clone();
        tokio::task::spawn(async move {
            while signals.recv().await.is_some() {
                event_w
                    .send(Event::Resize(
                        terminal_size::terminal_size().map_or(
                            (24, 80),
                            |(
                                terminal_size::Width(w),
                                terminal_size::Height(h),
                            )| { (h, w) },
                        ),
                    ))
                    // event_w is never closed, so this can never fail
                    .unwrap();
            }
        });
    }

    {
        let event_w = event_w.clone();
        std::thread::spawn(move || {
            loop {
                event_w
                    .send(Event::Key(input.read_key()))
                    // event_w is never closed, so this can never fail
                    .unwrap();
            }
        });
    }

    {
        let event_w = event_w.clone();
        tokio::task::spawn(async move {
            loop {
                let mut buf = [0_u8; 4096];
                tokio::select! {
                    res = pty.read(&mut buf) => {
                        let res = res.map(|n| buf[..n].to_vec());
                        let err = res.is_err();
                        event_w
                            .send(Event::Stdout(res))
                            // event_w is never closed, so this can never fail
                            .unwrap();
                        if err {
                            eprintln!("pty read failed: {}", err);
                            break;
                        }
                    }
                    res = input_r.recv() => {
                        // input_r is never closed, so this can never fail
                        let bytes: Vec<u8> = res.unwrap();
                        if let Err(e) = pty.write(&bytes).await {
                            event_w
                                .send(Event::Error(anyhow::anyhow!(e)))
                                // event_w is never closed, so this can never
                                // fail
                                .unwrap();
                        }
                    }
                    res = resize_r.recv() => {
                        // resize_r is never closed, so this can never fail
                        let size: (u16, u16) = res.unwrap();
                        if let Err(e) = pty.resize(
                            pty_process::Size::new(size.0, size.1),
                        ) {
                            event_w
                                .send(Event::Error(anyhow::anyhow!(e)))
                                // event_w is never closed, so this can never
                                // fail
                                .unwrap();
                        }
                    }
                    _ = child.wait() => {
                        event_w.send(Event::Quit).unwrap();
                        break;
                    }
                }
            }
        });
    }

    let mut writer = ttyrec::Writer::new(fh);
    loop {
        // XXX unwrap
        match event_r.recv().await.unwrap() {
            Event::Key(key) => {
                let key = key?;
                if let Some(key) = key {
                    input_w.send(key.into_bytes()).unwrap();
                } else {
                    break;
                }
            }
            Event::Stdout(bytes) => match bytes {
                Ok(bytes) => {
                    writer.frame(&bytes).await?;
                    stdout.write_all(&bytes).await?;
                    stdout.flush().await?;
                }
                Err(e) => {
                    anyhow::bail!("failed to read from child process: {}", e);
                }
            },
            Event::Resize((h, w)) => {
                resize_w.send((h, w)).unwrap();
            }
            Event::Error(e) => {
                return Err(e);
            }
            Event::Quit => break,
        }
    }

    Ok(())
}

#[paw::main]
fn main(opt: Opt) {
    match async_main(opt) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("ttyrec: {}", e);
            std::process::exit(1);
        }
    };
}

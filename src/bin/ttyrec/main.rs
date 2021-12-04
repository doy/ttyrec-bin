use async_std::io::{ReadExt as _, WriteExt as _};
use async_std::prelude::FutureExt as _;
use async_std::stream::StreamExt as _;
use pty_process::Command as _;

#[derive(Debug, structopt::StructOpt)]
#[structopt(about = "ttyrec")]
struct Opt {
    #[structopt(short, long, default_value = "ttyrec")]
    file: std::ffi::OsString,
    #[structopt(short, long)]
    cmd: Option<std::ffi::OsString>,
}

fn get_cmd(
    cmd: Option<std::ffi::OsString>,
) -> (std::ffi::OsString, Vec<std::ffi::OsString>) {
    if let Some(cmd) = cmd {
        ("/bin/sh".into(), vec!["-c".into(), cmd])
    } else {
        let shell =
            std::env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
        (shell, vec![])
    }
}

enum Event {
    Key(textmode::Result<Option<textmode::Key>>),
    Stdout(std::io::Result<Vec<u8>>),
    Resize((u16, u16)),
}

async fn resize(event_w: &async_std::channel::Sender<Event>) {
    let size = terminal_size::terminal_size().map_or(
        (24, 80),
        |(terminal_size::Width(w), terminal_size::Height(h))| (h, w),
    );
    event_w.send(Event::Resize(size)).await.unwrap();
}

async fn async_main(opt: Opt) -> anyhow::Result<()> {
    let Opt { cmd, file } = opt;
    let (cmd, args) = get_cmd(cmd);

    let size = terminal_size::terminal_size().map_or(
        (24, 80),
        |(terminal_size::Width(w), terminal_size::Height(h))| (h, w),
    );
    let fh = async_std::fs::File::create(file).await?;
    let mut input = textmode::Input::new().await?;
    let _input_guard = input.take_raw_guard();
    let mut stdout = async_std::io::stdout();
    let child = async_std::process::Command::new(cmd)
        .args(args)
        .spawn_pty(Some(&pty_process::Size::new(size.0, size.1)))?;

    let (event_w, event_r) = async_std::channel::unbounded();
    let (input_w, input_r) = async_std::channel::unbounded();
    let (resize_w, resize_r) = async_std::channel::unbounded();

    {
        let mut signals = signal_hook_async_std::Signals::new(&[
            signal_hook::consts::signal::SIGWINCH,
        ])?;
        let event_w = event_w.clone();
        async_std::task::spawn(async move {
            while signals.next().await.is_some() {
                resize(&event_w).await;
            }
        });
    }

    {
        let event_w = event_w.clone();
        async_std::task::spawn(async move {
            loop {
                event_w
                    .send(Event::Key(input.read_key().await))
                    .await
                    .unwrap();
            }
        });
    }

    {
        let event_w = event_w.clone();
        async_std::task::spawn(async move {
            loop {
                enum Res {
                    Read(Result<usize, std::io::Error>),
                    Write(Result<Vec<u8>, async_std::channel::RecvError>),
                    Resize(Result<(u16, u16), async_std::channel::RecvError>),
                }
                let mut buf = [0_u8; 4096];
                let mut pty = child.pty();
                let read = async { Res::Read(pty.read(&mut buf).await) };
                let write = async { Res::Write(input_r.recv().await) };
                let resize = async { Res::Resize(resize_r.recv().await) };
                match read.race(write).race(resize).await {
                    Res::Read(res) => {
                        let res = res.map(|n| buf[..n].to_vec());
                        let err = res.is_err();
                        event_w.send(Event::Stdout(res)).await.unwrap();
                        if err {
                            break;
                        }
                    }
                    Res::Write(res) => {
                        let bytes = res.unwrap();
                        pty.write(&bytes).await.unwrap();
                    }
                    Res::Resize(res) => {
                        let size = res.unwrap();
                        child
                            .resize_pty(&pty_process::Size::new(
                                size.0, size.1,
                            ))
                            .unwrap();
                    }
                }
            }
        });
    }

    resize(&event_w).await;

    let mut writer = ttyrec::Writer::new(fh);
    loop {
        match event_r.recv().await? {
            Event::Key(key) => {
                let key = key?;
                if let Some(key) = key {
                    input_w.send(key.into_bytes()).await?;
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
                    if e.raw_os_error() == Some(libc::EIO) {
                        break;
                    } else {
                        anyhow::bail!(
                            "failed to read from child process: {}",
                            e
                        );
                    }
                }
            },
            Event::Resize((h, w)) => {
                resize_w.send((h, w)).await?;
            }
        }
    }

    Ok(())
}

#[paw::main]
fn main(opt: Opt) {
    match async_std::task::block_on(async_main(opt)) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("ttyrec: {}", e);
            std::process::exit(1);
        }
    };
}

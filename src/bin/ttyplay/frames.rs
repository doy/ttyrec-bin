#[derive(Debug, Clone)]
pub struct Frame {
    screen: vt100::Screen,
    delay: std::time::Duration,
}

impl Frame {
    pub fn new(screen: vt100::Screen, delay: std::time::Duration) -> Self {
        Self { screen, delay }
    }

    pub fn into_screen(self) -> vt100::Screen {
        self.screen
    }

    pub fn delay(&self) -> std::time::Duration {
        self.delay
    }
}

pub struct FrameData {
    frames: Vec<Frame>,
    done_reading: bool,
    new_frame_w: tokio::sync::watch::Sender<Option<usize>>,
    new_frame_r: tokio::sync::watch::Receiver<Option<usize>>,
}

impl FrameData {
    pub fn new() -> Self {
        let (new_frame_w, new_frame_r) = tokio::sync::watch::channel(Some(0));
        Self {
            frames: vec![],
            done_reading: false,
            new_frame_w,
            new_frame_r,
        }
    }

    pub fn get(&self, i: usize) -> Option<&Frame> {
        self.frames.get(i)
    }

    pub fn count(&self) -> usize {
        self.frames.len()
    }

    pub fn search(
        &self,
        start: usize,
        query: &str,
        backwards: bool,
    ) -> Option<usize> {
        if backwards {
            self.frames
                .iter()
                .enumerate()
                .rev()
                .skip(self.frames.len() - start + 1)
                .find(|(_, frame)| frame.screen.contents().contains(query))
                .map(|(i, _)| i)
        } else {
            self.frames
                .iter()
                .enumerate()
                .skip(start)
                .find(|(_, frame)| frame.screen.contents().contains(query))
                .map(|(i, _)| i)
        }
    }

    pub fn add_frame(&mut self, frame: Frame) {
        self.frames.push(frame);
        self.new_frame_w
            .send(Some(self.frames.len()))
            // new_frame_w is never closed, so this can never fail
            .unwrap();
    }

    pub fn done_reading(&mut self) {
        self.done_reading = true;
        self.new_frame_w
            .send(None)
            // new_frame_w is never closed, so this can never fail
            .unwrap();
    }

    pub fn wait_for_frame(
        &self,
        i: usize,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = bool> + 'static + Send>,
    > {
        if i < self.frames.len() {
            return Box::pin(std::future::ready(true));
        }
        if self.done_reading {
            return Box::pin(std::future::ready(false));
        }
        let mut new_frame_r = self.new_frame_r.clone();
        Box::pin(async move {
            while new_frame_r.changed().await.is_ok() {
                if let Some(new_len) = *new_frame_r.borrow() {
                    if i < new_len {
                        return true;
                    }
                } else {
                    break;
                }
            }
            false
        })
    }
}

pub fn load_from_file(
    frames: std::sync::Arc<tokio::sync::Mutex<FrameData>>,
    fh: tokio::fs::File,
    event_w: tokio::sync::mpsc::UnboundedSender<crate::event::Event>,
    clamp: Option<u64>,
) {
    let clamp = clamp.map(std::time::Duration::from_millis);
    tokio::task::spawn(async move {
        let mut reader = ttyrec::Reader::new(fh);
        let size = terminal_size::terminal_size().map_or(
            (24, 80),
            |(terminal_size::Width(w), terminal_size::Height(h))| (h, w),
        );
        let mut parser = vt100::Parser::new(size.0, size.1, 0);
        let mut prev_delay = std::time::Duration::from_secs(0);
        let mut clamped_amount = std::time::Duration::from_secs(0);
        while let Ok(frame) = reader.read_frame().await {
            let mut delay = reader.offset().map_or_else(
                || std::time::Duration::from_secs(0),
                |offset| frame.time - offset - clamped_amount,
            );
            if let Some(clamp) = clamp {
                let clamped_delay = delay.min(prev_delay + clamp);
                if clamped_delay < delay {
                    clamped_amount += delay - clamped_delay;
                    delay = clamped_delay;
                }
            }
            parser.process(&frame.data);
            let mut frames = frames.clone().lock_owned().await;
            frames.add_frame(Frame::new(parser.screen().clone(), delay));
            event_w
                .send(crate::event::Event::FrameLoaded(Some(frames.count())))
                // event_w is never closed, so this can never fail
                .unwrap();
            prev_delay = delay;
        }
        frames.lock_owned().await.done_reading();
        event_w
            .send(crate::event::Event::FrameLoaded(None))
            // event_w is never closed, so this can never fail
            .unwrap();
    });
}

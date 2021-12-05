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
    new_frame_w: async_std::channel::Sender<Option<usize>>,
    new_frame_r: async_std::channel::Receiver<Option<usize>>,
}

impl FrameData {
    pub fn new() -> Self {
        let (new_frame_w, new_frame_r) = async_std::channel::unbounded();
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

    pub async fn add_frame(&mut self, frame: Frame) {
        self.frames.push(frame);
        self.new_frame_w
            .send(Some(self.frames.len()))
            .await
            .unwrap();
    }

    pub async fn done_reading(&mut self) {
        self.done_reading = true;
        self.new_frame_w.send(None).await.unwrap();
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

pub fn load_from_file(
    frames: async_std::sync::Arc<async_std::sync::Mutex<FrameData>>,
    fh: async_std::fs::File,
    event_w: async_std::channel::Sender<crate::event::Event>,
    clamp: Option<u64>,
) {
    let clamp = clamp.map(std::time::Duration::from_millis);
    async_std::task::spawn(async move {
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
            let mut frames = frames.lock_arc().await;
            frames
                .add_frame(Frame::new(parser.screen().clone(), delay))
                .await;
            event_w
                .send(crate::event::Event::FrameLoaded(Some(frames.count())))
                .await
                .unwrap();
            prev_delay = delay;
        }
        frames.lock_arc().await.done_reading().await;
        event_w
            .send(crate::event::Event::FrameLoaded(None))
            .await
            .unwrap();
    });
}

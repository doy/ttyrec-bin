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
            .unwrap()
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

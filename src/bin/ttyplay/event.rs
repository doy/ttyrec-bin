pub enum Event {
    FrameTransition((usize, vt100::Screen)),
    FrameLoaded(Option<usize>),
    Paused(bool),
    TimerAction(TimerAction),
    ToggleUi,
    Quit,
}

pub enum TimerAction {
    Pause,
    FirstFrame,
    LastFrame,
    NextFrame,
    PreviousFrame,
    Quit,
}

struct Reader {
    pending: async_std::sync::Mutex<Pending>,
    cvar: async_std::sync::Condvar,
}

impl Reader {
    fn new(
        input: async_std::channel::Receiver<Event>,
    ) -> async_std::sync::Arc<Self> {
        let this = Self {
            pending: async_std::sync::Mutex::new(Pending::new()),
            cvar: async_std::sync::Condvar::new(),
        };
        let this = async_std::sync::Arc::new(this);
        {
            let this = this.clone();
            async_std::task::spawn(async move {
                while let Ok(event) = input.recv().await {
                    this.event(event).await;
                }
                this.event(Event::Quit).await;
            });
        }
        this
    }

    async fn read(&self) -> Option<Event> {
        let mut pending = self
            .cvar
            .wait_until(self.pending.lock().await, |pending| {
                pending.has_event()
            })
            .await;
        pending.get_event()
    }

    async fn event(&self, event: Event) {
        let mut pending = self.pending.lock().await;
        pending.event(event);
        self.cvar.notify_one();
    }
}

#[derive(Default)]
struct Pending {
    render: Option<(usize, vt100::Screen)>,
    frame_loaded: Option<usize>,
    done_loading: bool,
    paused: Option<bool>,
    timer_actions: std::collections::VecDeque<TimerAction>,
    toggle_ui: bool,
    quit: bool,
}

impl Pending {
    fn new() -> Self {
        Self::default()
    }

    fn event(&mut self, event: Event) {
        match event {
            Event::FrameTransition((idx, screen)) => {
                self.render = Some((idx, screen));
            }
            Event::FrameLoaded(idx) => {
                if let Some(idx) = idx {
                    self.frame_loaded = Some(idx);
                } else {
                    self.done_loading = true;
                }
            }
            Event::Paused(paused) => {
                self.paused = Some(paused);
            }
            Event::TimerAction(action) => {
                self.timer_actions.push_back(action);
            }
            Event::ToggleUi => {
                self.toggle_ui = !self.toggle_ui;
            }
            Event::Quit => {
                self.quit = true;
            }
        }
    }

    fn has_event(&self) -> bool {
        self.render.is_some()
            || self.frame_loaded.is_some()
            || self.done_loading
            || self.paused.is_some()
            || !self.timer_actions.is_empty()
            || self.toggle_ui
            || self.quit
    }

    fn get_event(&mut self) -> Option<Event> {
        if self.quit {
            self.quit = false;
            Some(Event::Quit)
        } else if let Some(action) = self.timer_actions.pop_front() {
            Some(Event::TimerAction(action))
        } else if self.toggle_ui {
            self.toggle_ui = false;
            Some(Event::ToggleUi)
        } else if let Some(paused) = self.paused.take() {
            Some(Event::Paused(paused))
        } else if let Some(frame) = self.frame_loaded.take() {
            Some(Event::FrameLoaded(Some(frame)))
        } else if self.done_loading {
            self.done_loading = false;
            Some(Event::FrameLoaded(None))
        } else if let Some((idx, screen)) = self.render.take() {
            Some(Event::FrameTransition((idx, screen)))
        } else {
            None
        }
    }
}

pub async fn handle_events(
    event_r: async_std::channel::Receiver<Event>,
    timer_w: async_std::channel::Sender<TimerAction>,
    mut output: textmode::Output,
    pause_at_start: bool,
) -> anyhow::Result<()> {
    let mut display = crate::display::Display::new();
    if pause_at_start {
        display.paused(true);
    }
    let mut current_screen = vt100::Parser::default().screen().clone();
    let events = Reader::new(event_r);
    while let Some(event) = events.read().await {
        match event {
            Event::TimerAction(action) => {
                timer_w.send(action).await?;
                continue;
            }
            Event::FrameTransition((idx, screen)) => {
                current_screen = screen;
                display.current_frame(idx);
            }
            Event::FrameLoaded(n) => {
                if let Some(n) = n {
                    display.total_frames(n);
                } else {
                    display.done_loading();
                }
            }
            Event::Paused(paused) => {
                display.paused(paused);
            }
            Event::ToggleUi => {
                display.toggle_ui();
            }
            Event::Quit => {
                break;
            }
        }
        display.render(&current_screen, &mut output).await?;
    }

    Ok(())
}

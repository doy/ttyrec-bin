#[derive(Debug)]
pub enum Event {
    FrameTransition((usize, Box<vt100::Screen>)),
    FrameLoaded(Option<usize>),
    Paused(bool),
    Speed(u32),
    TimerAction(TimerAction),
    ToggleUi,
    ToggleHelp,
    ActiveSearch(String),
    CancelSearch,
    RunSearch(String, bool),
    Error(anyhow::Error),
    Quit,
}

#[derive(Debug)]
pub enum TimerAction {
    Pause,
    FirstFrame,
    LastFrame,
    NextFrame,
    PreviousFrame,
    SpeedUp,
    SlowDown,
    DefaultSpeed,
    Search(String, bool),
    Quit,
}

struct Reader {
    pending: tokio::sync::Mutex<Pending>,
    // XXX not quite a condvar, but i think close enough as long as there is
    // only ever one consumer? see
    // https://github.com/tokio-rs/tokio/issues/3892
    cvar: tokio::sync::Notify,
}

impl Reader {
    fn new(
        mut input: tokio::sync::mpsc::UnboundedReceiver<Event>,
    ) -> std::sync::Arc<Self> {
        let this = Self {
            pending: tokio::sync::Mutex::new(Pending::new()),
            cvar: tokio::sync::Notify::new(),
        };
        let this = std::sync::Arc::new(this);
        {
            let this = this.clone();
            tokio::task::spawn(async move {
                while let Some(event) = input.recv().await {
                    this.event(event).await;
                }
                this.event(Event::Quit).await;
            });
        }
        this
    }

    async fn read(&self) -> Option<Event> {
        loop {
            let mut pending = self.pending.lock().await;
            if pending.has_event() {
                return pending.get_event();
            }
            drop(pending);
            self.cvar.notified().await;
        }
    }

    async fn event(&self, event: Event) {
        let mut pending = self.pending.lock().await;
        pending.event(event);
        self.cvar.notify_one();
    }
}

#[derive(Default)]
struct Pending {
    render: Option<(usize, Box<vt100::Screen>)>,
    frame_loaded: Option<usize>,
    done_loading: bool,
    paused: Option<bool>,
    speed: Option<u32>,
    timer_actions: std::collections::VecDeque<TimerAction>,
    toggle_ui: bool,
    toggle_help: bool,
    active_search: Option<String>,
    cancel_search: bool,
    run_search: Option<(String, bool)>,
    error: Option<anyhow::Error>,
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
            Event::Speed(speed) => {
                self.speed = Some(speed);
            }
            Event::TimerAction(action) => {
                self.timer_actions.push_back(action);
            }
            Event::ToggleUi => {
                self.toggle_ui = !self.toggle_ui;
            }
            Event::ToggleHelp => {
                self.toggle_help = !self.toggle_help;
            }
            Event::ActiveSearch(s) => {
                self.active_search = Some(s);
                self.cancel_search = false;
                self.run_search = None;
            }
            Event::CancelSearch => {
                self.active_search = None;
                self.cancel_search = true;
                self.run_search = None;
            }
            Event::RunSearch(s, backwards) => {
                self.active_search = None;
                self.cancel_search = false;
                self.run_search = Some((s, backwards));
            }
            Event::Error(e) => {
                self.error = Some(e);
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
            || self.speed.is_some()
            || !self.timer_actions.is_empty()
            || self.toggle_ui
            || self.toggle_help
            || self.active_search.is_some()
            || self.cancel_search
            || self.run_search.is_some()
            || self.error.is_some()
            || self.quit
    }

    fn get_event(&mut self) -> Option<Event> {
        if let Some(e) = self.error.take() {
            Some(Event::Error(e))
        } else if self.quit {
            self.quit = false;
            Some(Event::Quit)
        } else if let Some(action) = self.timer_actions.pop_front() {
            Some(Event::TimerAction(action))
        } else if let Some(active_search) = self.active_search.take() {
            Some(Event::ActiveSearch(active_search))
        } else if self.cancel_search {
            self.cancel_search = false;
            Some(Event::CancelSearch)
        } else if let Some((run_search, backwards)) = self.run_search.take() {
            Some(Event::RunSearch(run_search, backwards))
        } else if self.toggle_ui {
            self.toggle_ui = false;
            Some(Event::ToggleUi)
        } else if self.toggle_help {
            self.toggle_help = false;
            Some(Event::ToggleHelp)
        } else if let Some(paused) = self.paused.take() {
            Some(Event::Paused(paused))
        } else if let Some(speed) = self.speed.take() {
            Some(Event::Speed(speed))
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
    event_r: tokio::sync::mpsc::UnboundedReceiver<Event>,
    timer_w: tokio::sync::mpsc::UnboundedSender<TimerAction>,
    mut output: textmode::Output,
) -> anyhow::Result<()> {
    let mut display = crate::display::Display::new();
    let events = Reader::new(event_r);
    while let Some(event) = events.read().await {
        match event {
            Event::TimerAction(action) => {
                timer_w.send(action)?;
                continue;
            }
            Event::FrameTransition((idx, screen)) => {
                display.screen(*screen);
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
            Event::Speed(speed) => {
                display.speed(speed);
            }
            Event::ToggleUi => {
                display.toggle_ui();
            }
            Event::ToggleHelp => {
                display.toggle_help();
            }
            Event::ActiveSearch(s) => {
                display.active_search(s);
            }
            Event::CancelSearch => {
                display.clear_search();
            }
            Event::RunSearch(s, backwards) => {
                display.clear_search();
                timer_w.send(TimerAction::Search(s, backwards))?;
            }
            Event::Error(e) => {
                return Err(e);
            }
            Event::Quit => {
                break;
            }
        }
        display.render(&mut output).await?;
    }

    Ok(())
}

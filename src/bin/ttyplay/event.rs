pub enum Event {
    Render((usize, vt100::Screen)),
    Key(textmode::Key),
    FrameLoaded(Option<usize>),
    Pause,
    Paused(bool),
    FirstFrame,
    LastFrame,
    NextFrame,
    PreviousFrame,
    Quit,
}

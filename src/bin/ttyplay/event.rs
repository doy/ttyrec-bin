pub enum Event {
    Render(vt100::Screen),
    Key(textmode::Key),
    Pause,
    Quit,
}

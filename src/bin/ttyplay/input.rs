pub fn to_event(key: &textmode::Key) -> Option<crate::event::Event> {
    Some(match key {
        textmode::Key::Char('g' | '0' | ')') => {
            crate::event::Event::FirstFrame
        }
        textmode::Key::Char('G' | '$') => crate::event::Event::LastFrame,
        textmode::Key::Char('l' | 'n') => crate::event::Event::NextFrame,
        textmode::Key::Char('h' | 'p') => crate::event::Event::PreviousFrame,
        textmode::Key::Char('q') => crate::event::Event::Quit,
        textmode::Key::Char(' ') => crate::event::Event::Pause,
        textmode::Key::Ctrl(b'i') => crate::event::Event::ToggleUi,
        _ => return None,
    })
}

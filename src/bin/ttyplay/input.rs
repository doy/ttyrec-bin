pub fn spawn_task(
    event_w: async_std::channel::Sender<crate::event::Event>,
    mut input: textmode::Input,
) {
    async_std::task::spawn(async move {
        while let Some(key) = input.read_key().await.unwrap() {
            let event = match key {
                textmode::Key::Char('g' | '0' | ')') => {
                    crate::event::Event::TimerAction(
                        crate::event::TimerAction::FirstFrame,
                    )
                }
                textmode::Key::Char('G' | '$') => {
                    crate::event::Event::TimerAction(
                        crate::event::TimerAction::LastFrame,
                    )
                }
                textmode::Key::Char('l' | 'n') => {
                    crate::event::Event::TimerAction(
                        crate::event::TimerAction::NextFrame,
                    )
                }
                textmode::Key::Char('h' | 'p') => {
                    crate::event::Event::TimerAction(
                        crate::event::TimerAction::PreviousFrame,
                    )
                }
                textmode::Key::Char('q') => crate::event::Event::Quit,
                textmode::Key::Char(' ') => crate::event::Event::TimerAction(
                    crate::event::TimerAction::Pause,
                ),
                textmode::Key::Ctrl(b'i') => crate::event::Event::ToggleUi,
                textmode::Key::Char('?') => crate::event::Event::ToggleHelp,
                textmode::Key::Char('+') => crate::event::Event::TimerAction(
                    crate::event::TimerAction::SpeedUp,
                ),
                textmode::Key::Char('-') => crate::event::Event::TimerAction(
                    crate::event::TimerAction::SlowDown,
                ),
                textmode::Key::Char('=') => crate::event::Event::TimerAction(
                    crate::event::TimerAction::DefaultSpeed,
                ),
                _ => continue,
            };
            event_w.send(event).await.unwrap();
        }
    });
}

pub async fn handle_input(
    key: textmode::Key,
    event_w: async_std::channel::Sender<crate::event::Event>,
) -> anyhow::Result<()> {
    match key {
        textmode::Key::Char('g' | '0' | ')') => {
            event_w.send(crate::event::Event::FirstFrame).await?;
        }
        textmode::Key::Char('G' | '$') => {
            event_w.send(crate::event::Event::LastFrame).await?;
        }
        textmode::Key::Char('l' | 'n') => {
            event_w.send(crate::event::Event::NextFrame).await?;
        }
        textmode::Key::Char('h' | 'p') => {
            event_w.send(crate::event::Event::PreviousFrame).await?;
        }
        textmode::Key::Char('q') => {
            event_w.send(crate::event::Event::Quit).await?;
        }
        textmode::Key::Char(' ') => {
            event_w.send(crate::event::Event::Pause).await?;
        }
        _ => {}
    }
    Ok(())
}

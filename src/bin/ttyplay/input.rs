pub async fn handle_input(
    key: textmode::Key,
    event_w: async_std::channel::Sender<crate::event::Event>,
) -> anyhow::Result<()> {
    match key {
        textmode::Key::Char('q') => {
            event_w.send(crate::event::Event::Quit).await?
        }
        textmode::Key::Char(' ') => {
            event_w.send(crate::event::Event::Pause).await?;
        }
        _ => {}
    }
    Ok(())
}

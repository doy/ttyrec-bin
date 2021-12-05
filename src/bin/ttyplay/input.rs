pub fn spawn_task(
    event_w: async_std::channel::Sender<crate::event::Event>,
    mut input: textmode::Input,
) {
    async_std::task::spawn(async move {
        let mut search: Option<String> = None;
        let mut prev_search = None;
        while let Some(key) = input.read_key().await.unwrap() {
            if let Some(ref mut search_contents) = search {
                match key {
                    textmode::Key::Char(c) => {
                        search_contents.push(c);
                        event_w
                            .send(crate::event::Event::ActiveSearch(
                                search_contents.clone(),
                            ))
                            .await
                            .unwrap();
                    }
                    textmode::Key::Backspace => {
                        search_contents.pop();
                        event_w
                            .send(crate::event::Event::ActiveSearch(
                                search_contents.clone(),
                            ))
                            .await
                            .unwrap();
                    }
                    textmode::Key::Ctrl(b'm') => {
                        event_w
                            .send(crate::event::Event::RunSearch(
                                search_contents.clone(),
                            ))
                            .await
                            .unwrap();
                        prev_search = search;
                        search = None;
                    }
                    textmode::Key::Escape => {
                        event_w
                            .send(crate::event::Event::CancelSearch)
                            .await
                            .unwrap();
                        search = None;
                    }
                    _ => {}
                }
            } else {
                let event = match key {
                    textmode::Key::Char('0') => {
                        crate::event::Event::TimerAction(
                            crate::event::TimerAction::FirstFrame,
                        )
                    }
                    textmode::Key::Char('$') => {
                        crate::event::Event::TimerAction(
                            crate::event::TimerAction::LastFrame,
                        )
                    }
                    textmode::Key::Char('l') => {
                        crate::event::Event::TimerAction(
                            crate::event::TimerAction::NextFrame,
                        )
                    }
                    textmode::Key::Char('h') => {
                        crate::event::Event::TimerAction(
                            crate::event::TimerAction::PreviousFrame,
                        )
                    }
                    textmode::Key::Char('q') => crate::event::Event::Quit,
                    textmode::Key::Char(' ') => {
                        crate::event::Event::TimerAction(
                            crate::event::TimerAction::Pause,
                        )
                    }
                    textmode::Key::Ctrl(b'i') => {
                        crate::event::Event::ToggleUi
                    }
                    textmode::Key::Char('?') => {
                        crate::event::Event::ToggleHelp
                    }
                    textmode::Key::Char('+') => {
                        crate::event::Event::TimerAction(
                            crate::event::TimerAction::SpeedUp,
                        )
                    }
                    textmode::Key::Char('-') => {
                        crate::event::Event::TimerAction(
                            crate::event::TimerAction::SlowDown,
                        )
                    }
                    textmode::Key::Char('=') => {
                        crate::event::Event::TimerAction(
                            crate::event::TimerAction::DefaultSpeed,
                        )
                    }
                    textmode::Key::Char('/') => {
                        search = Some("".to_string());
                        crate::event::Event::ActiveSearch("".to_string())
                    }
                    textmode::Key::Char('n') => {
                        if let Some(ref search) = prev_search {
                            crate::event::Event::RunSearch(search.clone())
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                };
                event_w.send(event).await.unwrap();
            }
        }
    });
}

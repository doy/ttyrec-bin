pub fn spawn_thread(
    event_w: tokio::sync::mpsc::UnboundedSender<crate::event::Event>,
    mut input: textmode::blocking::Input,
) {
    std::thread::spawn(move || {
        let mut search: Option<String> = None;
        let mut prev_search = None;
        loop {
            let key = match input.read_key() {
                Ok(Some(key)) => key,
                Ok(None) => break,
                Err(e) => {
                    event_w
                        .send(crate::event::Event::Error(anyhow::anyhow!(e)))
                        // event_w is never closed, so this can never fail
                        .unwrap();
                    break;
                }
            };
            if let Some(ref mut search_contents) = search {
                match key {
                    textmode::Key::Char(c) => {
                        search_contents.push(c);
                        event_w
                            .send(crate::event::Event::ActiveSearch(
                                search_contents.clone(),
                            ))
                            // event_w is never closed, so this can never fail
                            .unwrap();
                    }
                    textmode::Key::Backspace => {
                        search_contents.pop();
                        event_w
                            .send(crate::event::Event::ActiveSearch(
                                search_contents.clone(),
                            ))
                            // event_w is never closed, so this can never fail
                            .unwrap();
                    }
                    textmode::Key::Ctrl(b'm') => {
                        event_w
                            .send(crate::event::Event::RunSearch(
                                search_contents.clone(),
                                false,
                            ))
                            // event_w is never closed, so this can never fail
                            .unwrap();
                        prev_search = search;
                        search = None;
                    }
                    textmode::Key::Escape => {
                        event_w
                            .send(crate::event::Event::CancelSearch)
                            // event_w is never closed, so this can never fail
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
                        search = Some(String::new());
                        crate::event::Event::ActiveSearch(String::new())
                    }
                    textmode::Key::Char('n') => {
                        if let Some(ref search) = prev_search {
                            crate::event::Event::RunSearch(
                                search.clone(),
                                false,
                            )
                        } else {
                            continue;
                        }
                    }
                    textmode::Key::Char('p') => {
                        if let Some(ref search) = prev_search {
                            crate::event::Event::RunSearch(
                                search.clone(),
                                true,
                            )
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                };
                // event_w is never closed, so this can never fail
                event_w.send(event).unwrap();
            }
        }
    });
}

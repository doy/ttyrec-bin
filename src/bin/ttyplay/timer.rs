pub fn spawn_task(
    event_w: tokio::sync::mpsc::UnboundedSender<crate::event::Event>,
    frames: std::sync::Arc<tokio::sync::Mutex<crate::frames::FrameData>>,
    mut timer_r: tokio::sync::mpsc::UnboundedReceiver<
        crate::event::TimerAction,
    >,
    pause_at_start: bool,
    speed: u32,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        let mut idx = 0;
        let mut start_time = std::time::Instant::now();
        let mut paused_time = if pause_at_start {
            event_w
                .send(crate::event::Event::Paused(true))
                // event_w is never closed, so this can never fail
                .unwrap();
            Some(start_time)
        } else {
            None
        };
        let mut force_update_time = false;
        let mut playback_ratio = 2_u32.pow(speed);
        loop {
            let wait = async {
                let wait_read =
                    frames.clone().lock_owned().await.wait_for_frame(idx);
                if wait_read.await {
                    let frame = frames
                        .clone()
                        .lock_owned()
                        .await
                        .get(idx)
                        .unwrap()
                        .clone();
                    if force_update_time {
                        let now = std::time::Instant::now();
                        start_time = now - frame.delay() * playback_ratio / 16
                            // give a bit of extra time before moving to the
                            // next frame, otherwise backing up behind two
                            // frames that are extremely close together
                            // doesn't work
                            + std::time::Duration::from_millis(200);
                        if paused_time.take().is_some() {
                            paused_time = Some(now);
                        }
                        force_update_time = false;
                    } else if paused_time.is_some() {
                        std::future::pending::<()>().await;
                    } else {
                        tokio::time::sleep(
                            (start_time
                                + frame.delay() * playback_ratio / 16)
                                .saturating_duration_since(
                                    std::time::Instant::now(),
                                ),
                        )
                        .await;
                    }
                    Some(Box::new(frame.into_screen()))
                } else {
                    None
                }
            };
            tokio::select! {
                screen = wait => if let Some(screen) = screen {
                    event_w
                        .send(crate::event::Event::FrameTransition((
                            idx, screen,
                        )))
                        // event_w is never closed, so this can never fail
                        .unwrap();
                    idx += 1;
                }
                else {
                    idx = frames.clone().lock_owned().await.count() - 1;
                    paused_time = Some(std::time::Instant::now());
                    event_w
                        .send(crate::event::Event::Paused(true))
                        // event_w is never closed, so this can never fail
                        .unwrap();
                },
                action = timer_r.recv() => match action {
                    Some(action) => match action {
                        crate::event::TimerAction::Pause => {
                            let now = std::time::Instant::now();
                            paused_time.take().map_or_else(|| {
                                paused_time = Some(now);
                            }, |time| {
                                start_time += now - time;
                            });
                            event_w
                                .send(crate::event::Event::Paused(
                                    paused_time.is_some(),
                                ))
                                // event_w is never closed, so this can never
                                // fail
                                .unwrap();
                        }
                        crate::event::TimerAction::FirstFrame => {
                            idx = 0;
                            force_update_time = true;
                        }
                        crate::event::TimerAction::LastFrame => {
                            idx =
                                frames.clone().lock_owned().await.count() - 1;
                            force_update_time = true;
                        }
                        // force_update_time will immediately transition to the
                        // next frame and do idx += 1 on its own
                        crate::event::TimerAction::NextFrame => {
                            force_update_time = true;
                        }
                        crate::event::TimerAction::PreviousFrame => {
                            idx = idx.saturating_sub(2);
                            force_update_time = true;
                        }
                        crate::event::TimerAction::SpeedUp => {
                            if playback_ratio > 1 {
                                playback_ratio /= 2;
                                let now = std::time::Instant::now();
                                start_time = now - (now - start_time) / 2;
                                event_w
                                    .send(crate::event::Event::Speed(
                                        playback_ratio,
                                    ))
                                    // event_w is never closed, so this can
                                    // never fail
                                    .unwrap();
                            }
                        }
                        crate::event::TimerAction::SlowDown => {
                            if playback_ratio < 256 {
                                playback_ratio *= 2;
                                let now = std::time::Instant::now();
                                start_time = now - (now - start_time) * 2;
                                event_w
                                    .send(crate::event::Event::Speed(
                                        playback_ratio,
                                    ))
                                    // event_w is never closed, so this can
                                    // never fail
                                    .unwrap();
                            }
                        }
                        crate::event::TimerAction::DefaultSpeed => {
                            let now = std::time::Instant::now();
                            start_time = now
                                - (((now - start_time) * 16) / playback_ratio);
                            playback_ratio = 16;
                            event_w
                                .send(
                                    crate::event::Event::Speed(playback_ratio)
                                )
                                // event_w is never closed, so this can never
                                // fail
                                .unwrap();
                        }
                        crate::event::TimerAction::Search(s, backwards) => {
                            if let Some(new_idx) =
                                frames.clone()
                                    .lock_owned()
                                    .await
                                    .search(idx, &s, backwards)
                            {
                                idx = new_idx;
                                force_update_time = true;
                            }
                        }
                        crate::event::TimerAction::Quit => break,
                    }
                    None => unreachable!(),
                },
            }
        }
    })
}

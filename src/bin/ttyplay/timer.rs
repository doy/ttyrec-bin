use async_std::prelude::FutureExt as _;

pub fn spawn_task(
    event_w: async_std::channel::Sender<crate::event::Event>,
    frames: async_std::sync::Arc<
        async_std::sync::Mutex<crate::frames::FrameData>,
    >,
    timer_r: async_std::channel::Receiver<crate::event::TimerAction>,
    pause_at_start: bool,
    speed: u32,
) -> async_std::task::JoinHandle<()> {
    async_std::task::spawn(async move {
        let mut idx = 0;
        let mut start_time = std::time::Instant::now();
        let mut paused_time = if pause_at_start {
            event_w
                .send(crate::event::Event::Paused(true))
                .await
                .unwrap();
            Some(start_time)
        } else {
            None
        };
        let mut force_update_time = false;
        let mut playback_ratio = 2_u32.pow(speed);
        loop {
            enum Res {
                Wait(Option<vt100::Screen>),
                TimerAction(
                    Result<
                        crate::event::TimerAction,
                        async_std::channel::RecvError,
                    >,
                ),
            }
            let wait = async {
                let wait_read = frames.lock_arc().await.wait_for_frame(idx);
                if wait_read.await {
                    let frame =
                        frames.lock_arc().await.get(idx).unwrap().clone();
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
                        async_std::task::sleep(
                            (start_time
                                + frame.delay() * playback_ratio / 16)
                                .saturating_duration_since(
                                    std::time::Instant::now(),
                                ),
                        )
                        .await;
                    }
                    Res::Wait(Some(frame.into_screen()))
                } else {
                    Res::Wait(None)
                }
            };
            let action = async { Res::TimerAction(timer_r.recv().await) };
            match wait.race(action).await {
                Res::Wait(Some(screen)) => {
                    event_w
                        .send(crate::event::Event::FrameTransition((
                            idx, screen,
                        )))
                        .await
                        .unwrap();
                    idx += 1;
                }
                Res::Wait(None) => {
                    idx = frames.lock_arc().await.count() - 1;
                    paused_time = Some(std::time::Instant::now());
                    event_w
                        .send(crate::event::Event::Paused(true))
                        .await
                        .unwrap();
                }
                Res::TimerAction(Ok(action)) => match action {
                    crate::event::TimerAction::Pause => {
                        let now = std::time::Instant::now();
                        if let Some(time) = paused_time.take() {
                            start_time += now - time;
                        } else {
                            paused_time = Some(now);
                        }
                        event_w
                            .send(crate::event::Event::Paused(
                                paused_time.is_some(),
                            ))
                            .await
                            .unwrap();
                    }
                    crate::event::TimerAction::FirstFrame => {
                        idx = 0;
                        force_update_time = true;
                    }
                    crate::event::TimerAction::LastFrame => {
                        idx = frames.lock_arc().await.count() - 1;
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
                                .await
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
                                .await
                                .unwrap();
                        }
                    }
                    crate::event::TimerAction::DefaultSpeed => {
                        let now = std::time::Instant::now();
                        start_time = now
                            - (((now - start_time) * 16) / playback_ratio);
                        playback_ratio = 16;
                        event_w
                            .send(crate::event::Event::Speed(playback_ratio))
                            .await
                            .unwrap();
                    }
                    crate::event::TimerAction::Search(s, backwards) => {
                        if let Some(new_idx) =
                            frames.lock_arc().await.search(idx, &s, backwards)
                        {
                            idx = new_idx;
                            force_update_time = true;
                        }
                    }
                    crate::event::TimerAction::Quit => break,
                },
                Res::TimerAction(Err(e)) => panic!("{}", e),
            }
        }
    })
}

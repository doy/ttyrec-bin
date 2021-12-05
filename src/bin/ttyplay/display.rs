use textmode::Textmode as _;

pub struct Display {
    screen: vt100::Screen,
    current_frame: usize,
    total_frames: usize,
    done_loading: bool,
    paused: bool,
    speed: u32,
    show_ui: bool,
    show_help: bool,
    active_search: Option<String>,
}

impl Display {
    pub fn new() -> Self {
        Self {
            screen: vt100::Parser::default().screen().clone(),
            current_frame: 0,
            total_frames: 0,
            done_loading: false,
            paused: false,
            speed: 16,
            show_ui: true,
            show_help: false,
            active_search: None,
        }
    }

    pub fn screen(&mut self, screen: vt100::Screen) {
        self.screen = screen;
    }

    pub fn current_frame(&mut self, idx: usize) {
        self.current_frame = idx;
    }

    pub fn total_frames(&mut self, n: usize) {
        self.total_frames = n;
    }

    pub fn done_loading(&mut self) {
        self.done_loading = true;
    }

    pub fn paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub fn speed(&mut self, speed: u32) {
        self.speed = speed;
    }

    pub fn toggle_ui(&mut self) {
        self.show_ui = !self.show_ui;
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn active_search(&mut self, s: String) {
        self.active_search = Some(s);
    }

    pub fn clear_search(&mut self) {
        self.active_search = None;
    }

    pub async fn render(
        &self,
        output: &mut textmode::Output,
    ) -> anyhow::Result<()> {
        let pos = output.screen().cursor_position();

        self.render_screen(output);

        if self.paused && self.show_ui {
            self.render_frame_count(output);
            self.render_speed(output);
            self.render_pause_symbol(output);

            if self.show_help {
                self.render_help(output);
            }
        }

        self.render_search(output);

        output.reset_attributes();
        output.move_to(pos.0, pos.1);
        output.refresh().await?;

        Ok(())
    }

    fn render_screen(&self, output: &mut textmode::Output) {
        output.clear();
        output.move_to(0, 0);
        output.write(&self.screen.contents_formatted());
    }

    fn render_frame_count(&self, output: &mut textmode::Output) {
        output.move_to(0, 0);
        output.reset_attributes();
        output.set_fgcolor(textmode::color::BLACK);
        if self.done_loading {
            output.set_bgcolor(textmode::color::CYAN);
        } else {
            output.set_bgcolor(textmode::color::RED);
        }
        output.write_str(&format!(
            " {}/{} ",
            self.current_frame + 1,
            self.total_frames
        ));
    }

    fn render_speed(&self, output: &mut textmode::Output) {
        if self.speed != 16 {
            output.move_to(1, 0);
            output.reset_attributes();
            output.set_fgcolor(textmode::color::BLACK);
            output.set_bgcolor(textmode::color::CYAN);

            output.write_str(&format!(
                "speed: {}x",
                16.0 / f64::from(self.speed)
            ));
        }
    }

    #[allow(clippy::unused_self)]
    fn render_pause_symbol(&self, output: &mut textmode::Output) {
        let size = output.screen().size();
        output.move_to(0, size.1 - 1);
        output.reset_attributes();
        output.set_fgcolor(textmode::color::BLACK);
        output.set_bgcolor(textmode::color::RED);
        output.write_str("\u{23f8}");
    }

    #[allow(clippy::unused_self)]
    fn render_help(&self, output: &mut textmode::Output) {
        let size = output.screen().size();
        output.reset_attributes();
        output.set_fgcolor(textmode::color::BLACK);
        output.set_bgcolor(textmode::color::CYAN);

        output.move_to(size.0 - 16, size.1 - 23);
        output.write_str("         keys          ");
        output.move_to(size.0 - 15, size.1 - 23);
        output.write_str(" q:     quit           ");
        output.move_to(size.0 - 14, size.1 - 23);
        output.write_str(" space: pause/unpause  ");
        output.move_to(size.0 - 13, size.1 - 23);
        output.write_str(" tab:   hide/show ui   ");
        output.move_to(size.0 - 12, size.1 - 23);
        output.write_str(" h:     previous frame ");
        output.move_to(size.0 - 11, size.1 - 23);
        output.write_str(" l:     next frame     ");
        output.move_to(size.0 - 10, size.1 - 23);
        output.write_str(" 0:     first frame    ");
        output.move_to(size.0 - 9, size.1 - 23);
        output.write_str(" $:     last frame     ");
        output.move_to(size.0 - 8, size.1 - 23);
        output.write_str(" +:     increase speed ");
        output.move_to(size.0 - 7, size.1 - 23);
        output.write_str(" -:     decrease speed ");
        output.move_to(size.0 - 6, size.1 - 23);
        output.write_str(" =:     normal speed   ");
        output.move_to(size.0 - 5, size.1 - 23);
        output.write_str(" /:     search         ");
        output.move_to(size.0 - 4, size.1 - 23);
        output.write_str(" n:     next match     ");
        output.move_to(size.0 - 3, size.1 - 23);
        output.write_str(" p:     previous match ");
        output.move_to(size.0 - 2, size.1 - 23);
        output.write_str(" ?:     hide/show help ");
    }

    fn render_search(&self, output: &mut textmode::Output) {
        if let Some(search) = &self.active_search {
            let size = output.screen().size();
            output.reset_attributes();
            output.set_fgcolor(textmode::color::BLACK);
            output.set_bgcolor(textmode::color::CYAN);

            output.move_to(size.0 - 1, 0);
            output.write_str("/");
            output.write_str(search);
            output.write_str(&" ".repeat(size.1 as usize - search.len() - 1));
        }
    }
}

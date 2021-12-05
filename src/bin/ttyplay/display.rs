use textmode::Textmode as _;

pub struct Display {
    current_frame: usize,
    total_frames: usize,
    done_loading: bool,
    paused: bool,
    show_ui: bool,
    show_help: bool,
}

impl Display {
    pub fn new() -> Self {
        Self {
            current_frame: 0,
            total_frames: 0,
            done_loading: false,
            paused: false,
            show_ui: true,
            show_help: false,
        }
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

    pub fn toggle_ui(&mut self) {
        self.show_ui = !self.show_ui;
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub async fn render(
        &self,
        screen: &vt100::Screen,
        output: &mut textmode::Output,
    ) -> anyhow::Result<()> {
        output.clear();
        output.move_to(0, 0);
        output.write(&screen.contents_formatted());
        if self.paused && self.show_ui {
            let pos = output.screen().cursor_position();

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

            let size = output.screen().size();
            output.move_to(0, size.1 - 1);
            output.reset_attributes();
            output.set_fgcolor(textmode::color::BLACK);
            output.set_bgcolor(textmode::color::RED);
            output.write_str("\u{23f8}");

            if self.show_help {
                output.reset_attributes();
                output.set_fgcolor(textmode::color::BLACK);
                output.set_bgcolor(textmode::color::CYAN);

                output.move_to(size.0 - 12, size.1 - 23);
                output.write_str("         keys          ");
                output.move_to(size.0 - 11, size.1 - 23);
                output.write_str(" q:     quit           ");
                output.move_to(size.0 - 10, size.1 - 23);
                output.write_str(" space: pause/unpause  ");
                output.move_to(size.0 - 9, size.1 - 23);
                output.write_str(" tab:   hide/show ui   ");
                output.move_to(size.0 - 8, size.1 - 23);
                output.write_str(" h/p:   previous frame ");
                output.move_to(size.0 - 7, size.1 - 23);
                output.write_str(" l/n:   next frame     ");
                output.move_to(size.0 - 6, size.1 - 23);
                output.write_str(" g/0:   first frame    ");
                output.move_to(size.0 - 5, size.1 - 23);
                output.write_str(" G/$:   last frame     ");
                output.move_to(size.0 - 4, size.1 - 23);
                output.write_str(" +:     increase speed ");
                output.move_to(size.0 - 3, size.1 - 23);
                output.write_str(" -:     decrease speed ");
                output.move_to(size.0 - 2, size.1 - 23);
                output.write_str(" =:     normal speed   ");
                output.move_to(size.0 - 1, size.1 - 23);
                output.write_str(" ?:     hide/show help ");
            }

            output.reset_attributes();
            output.move_to(pos.0, pos.1);
        }

        output.refresh().await?;

        Ok(())
    }
}

use textmode::Textmode as _;

pub struct Display {
    current_frame: usize,
    total_frames: usize,
    done_loading: bool,
    paused: bool,
    show_ui: bool,
}

impl Display {
    pub fn new() -> Self {
        Self {
            current_frame: 0,
            total_frames: 0,
            done_loading: false,
            paused: false,
            show_ui: true,
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

            output.reset_attributes();
            output.move_to(pos.0, pos.1);
        }

        output.refresh().await?;

        Ok(())
    }
}

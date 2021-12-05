use textmode::Textmode as _;

pub struct Display {
    current_frame: usize,
    total_frames: usize,
    done_loading: bool,
    paused: bool,
}

impl Display {
    pub fn new() -> Self {
        Self {
            current_frame: 0,
            total_frames: 0,
            done_loading: false,
            paused: false,
        }
    }

    pub fn current_frame(&mut self, idx: usize) {
        self.current_frame = idx;
    }

    pub fn get_current_frame(&self) -> usize {
        self.current_frame
    }

    pub fn total_frames(&mut self, n: usize) {
        self.total_frames = n;
    }

    pub fn get_total_frames(&self) -> usize {
        self.total_frames
    }

    pub fn done_loading(&mut self) {
        self.done_loading = true;
    }

    pub fn paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub async fn render(
        &self,
        screen: &vt100::Screen,
        output: &mut textmode::Output,
    ) -> anyhow::Result<()> {
        output.clear();
        output.move_to(0, 0);
        output.write(&screen.contents_formatted());
        if self.paused {
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
            output.write_str("⏸");

            output.reset_attributes();
            output.move_to(pos.0, pos.1);
        }

        output.refresh().await?;

        Ok(())
    }
}

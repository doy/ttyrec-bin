use textmode::Textmode as _;

pub struct Display {}

impl Display {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn render(
        &self,
        screen: &vt100::Screen,
        output: &mut textmode::Output,
    ) -> anyhow::Result<()> {
        output.clear();
        output.move_to(0, 0);
        output.write(&screen.contents_formatted());
        output.refresh().await?;
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    vcm_menubar::init_logging()?;
    vcm_menubar::run();
    Ok(())
}

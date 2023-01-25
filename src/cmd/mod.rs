mod video;

pub use video::*;

pub(crate) trait Cmd {
    fn run(self) -> anyhow::Result<()>;
}

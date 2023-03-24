mod video;

pub use video::*;
use async_trait::async_trait;

#[async_trait]
pub(crate) trait Cmd {
    async fn run(self) -> anyhow::Result<()>;
}

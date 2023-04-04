mod video;

use crate::prelude::*;
use async_trait::async_trait;

pub use video::*;

#[async_trait]
pub(crate) trait Cmd {
    async fn run(self) -> Result;
}

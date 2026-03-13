use clap::ValueEnum;
use serde::Serialize;

#[derive(Debug, Clone, ValueEnum, PartialEq, Eq, Serialize)]
pub enum Protocol {
    #[value(name = "uni-v2")]
    UniswapV2,
    #[value(name = "uni-v3")]
    UniswapV3,
    #[value(name = "uni-v4")]
    UniswapV4,
    #[value(name = "algebra-integral")]
    AlgebraIntegral,
}

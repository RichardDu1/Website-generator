pub mod fair_price;
pub mod inventory;
pub mod quoter;

/// Supported assets for 15-minute markets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Asset {
    BTC,
    ETH,
    SOL,
    XRP,
}

impl Asset {
    pub fn binance_symbol(&self) -> &'static str {
        match self {
            Asset::BTC => "btcusdt",
            Asset::ETH => "ethusdt",
            Asset::SOL => "solusdt",
            Asset::XRP => "xrpusdt",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Asset::BTC => "BTC/USD",
            Asset::ETH => "ETH/USD",
            Asset::SOL => "SOL/USD",
            Asset::XRP => "XRP/USD",
        }
    }

    pub fn all() -> &'static [Asset] {
        &[Asset::BTC, Asset::ETH, Asset::SOL, Asset::XRP]
    }
}

impl std::fmt::Display for Asset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

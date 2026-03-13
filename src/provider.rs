use alloy::providers::{
    Identity, ProviderBuilder, RootProvider,
    fillers::{BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller},
    layers::CallBatchProvider,
};

pub type MyProvider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        ChainIdFiller,
    >,
    CallBatchProvider<RootProvider>,
>;

pub async fn create_provider(
    rpc_url: &str,
) -> Result<MyProvider, alloy::transports::TransportError> {
    let provider = ProviderBuilder::new()
        .fetch_chain_id()
        .with_call_batching()
        // .network::<AnyNetwork>()
        .connect(rpc_url)
        .await?;
    Ok(provider)
}

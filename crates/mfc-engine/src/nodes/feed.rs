use mfc_core::error::EngineError;
use mfc_core::graph::{FeedParams, FlowBasis};
use mfc_core::stream::MaterialStream;
use mfc_core::traits::ComponentProvider;

pub fn compute_feed(
    params: &FeedParams,
    provider: &dyn ComponentProvider,
) -> Result<MaterialStream, EngineError> {
    let mw = provider.get_mw(&params.cas_number)?;

    let stream = match params.flow_basis {
        FlowBasis::Mass => {
            MaterialStream::from_pure_mass(&params.cas_number, params.total_flow, mw)
        }
        FlowBasis::Mole => {
            MaterialStream::from_pure_mole(&params.cas_number, params.total_flow, mw)
        }
    };

    Ok(stream)
}

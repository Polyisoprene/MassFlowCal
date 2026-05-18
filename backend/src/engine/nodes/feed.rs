use crate::db::Database;
use crate::error::EngineError;
use crate::models::graph::{FeedParams, FlowBasis};
use crate::models::stream::MaterialStream;

pub fn compute_feed(
    params: &FeedParams,
    db: &Database,
) -> Result<MaterialStream, EngineError> {
    let mw = db.get_mw(&params.cas_number)?;

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

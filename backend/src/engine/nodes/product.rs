use std::collections::HashMap;
use crate::error::EngineError;
use crate::models::stream::MaterialStream;

use super::NodeOutputs;

pub fn compute_product(
    inputs: &HashMap<String, MaterialStream>,
) -> Result<NodeOutputs, EngineError> {
    let stream = inputs
        .values()
        .next()
        .cloned()
        .unwrap_or_else(|| MaterialStream {
            mass_flow: 0.0,
            mole_flow: 0.0,
            components: HashMap::new(),
        });

    Ok(vec![("product_out".to_string(), stream)])
}

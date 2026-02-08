pub mod missing_access_control;
pub mod missing_addr_validate;
pub mod unbounded_iteration;

/// Returns all built-in detectors
pub fn all_detectors() -> Vec<Box<dyn cosmwasm_guard::detector::Detector>> {
    vec![
        Box::new(missing_addr_validate::MissingAddrValidate),
        Box::new(missing_access_control::MissingAccessControl),
        Box::new(unbounded_iteration::UnboundedIteration),
    ]
}

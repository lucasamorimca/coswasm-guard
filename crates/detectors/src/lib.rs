pub mod arithmetic_overflow;
pub mod incorrect_permission_hierarchy;
pub mod missing_access_control;
pub mod missing_addr_validate;
pub mod missing_error_propagation;
pub mod nondeterministic_iteration;
pub mod storage_key_collision;
pub mod submessage_reply;
pub mod unbounded_iteration;
pub mod unsafe_unwrap;

/// Returns all built-in detectors
pub fn all_detectors() -> Vec<Box<dyn cosmwasm_guard::detector::Detector>> {
    vec![
        Box::new(missing_addr_validate::MissingAddrValidate),
        Box::new(missing_access_control::MissingAccessControl),
        Box::new(unbounded_iteration::UnboundedIteration),
        Box::new(storage_key_collision::StorageKeyCollision),
        Box::new(unsafe_unwrap::UnsafeUnwrap),
        Box::new(arithmetic_overflow::ArithmeticOverflow),
        Box::new(missing_error_propagation::MissingErrorPropagation),
        Box::new(submessage_reply::SubmessageReplyUnvalidated),
        Box::new(nondeterministic_iteration::NondeterministicIteration),
        Box::new(incorrect_permission_hierarchy::IncorrectPermissionHierarchy),
    ]
}

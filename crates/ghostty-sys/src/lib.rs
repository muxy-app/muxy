pub mod ffi {
    #![allow(
        clippy::missing_safety_doc,
        clippy::too_many_arguments,
        clippy::undocumented_unsafe_blocks,
        missing_unsafe_on_extern,
        non_camel_case_types,
        non_snake_case,
        non_upper_case_globals,
        unsafe_code,
        unsafe_op_in_unsafe_fn
    )]

    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

use std::collections::{BTreeMap, HashMap};

use cairo_lang_casm::{hints::PythonicHint, instructions::Instruction};
use cairo_lang_sierra::extensions::{
    bitwise::BitwiseType, ec::EcOpType, pedersen::PedersenType, poseidon::PoseidonType,
    range_check::RangeCheckType, NamedType,
};
use cairo_vm::{
    serde::deserialize_program::{ApTracking, FlowTrackingData, HintParams},
    types::builtin_name::BuiltinName,
};

pub fn get_function_builtins(
    params: &[cairo_lang_sierra::ids::ConcreteTypeId],
) -> (
    Vec<BuiltinName>,
    HashMap<cairo_lang_sierra::ids::GenericTypeId, i16>,
) {
    let mut builtins = Vec::new();
    let mut builtin_offset: HashMap<cairo_lang_sierra::ids::GenericTypeId, i16> = HashMap::new();
    let mut current_offset = 3;
    for (debug_name, builtin_name, sierra_id) in [
        ("Poseidon", BuiltinName::poseidon, PoseidonType::ID),
        ("EcOp", BuiltinName::ec_op, EcOpType::ID),
        ("Bitwise", BuiltinName::bitwise, BitwiseType::ID),
        ("RangeCheck", BuiltinName::range_check, RangeCheckType::ID),
        ("Pedersen", BuiltinName::pedersen, PedersenType::ID),
    ] {
        if params
            .iter()
            .any(|id| id.debug_name.as_deref() == Some(debug_name))
        {
            builtins.push(builtin_name);
            builtin_offset.insert(sierra_id, current_offset);
            current_offset += 1;
        }
    }
    // builtins.push(BuiltinName::output);
    builtins.reverse();
    (builtins, builtin_offset)
}

pub fn build_hints_dict<'b>(
    instructions: impl Iterator<Item = &'b Instruction>,
) -> HashMap<usize, Vec<HintParams>> {
    let mut program_hints: HashMap<usize, Vec<HintParams>> = HashMap::new();

    let mut hint_offset = 0;

    for instruction in instructions {
        if !instruction.hints.is_empty() {
            program_hints.insert(
                hint_offset,
                instruction
                    .hints
                    .iter()
                    .map(|hint| hint.get_pythonic_hint())
                    .map(|code| HintParams {
                        code,
                        accessible_scopes: Vec::new(),
                        flow_tracking_data: FlowTrackingData {
                            ap_tracking: ApTracking::default(),
                            reference_ids: HashMap::new(),
                        },
                    })
                    .collect(),
            );
        }
        hint_offset += instruction.body.op_size();
    }
    program_hints
}

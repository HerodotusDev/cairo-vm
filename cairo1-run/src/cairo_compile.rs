pub mod segment;

use crate::error::Error;
use crate::{cairo_compile::segment::compute_bytecode_segment_lengths, cairo_run::find_function};
use cairo_felt::Felt252;
use cairo_lang_casm::assembler::AssembledCairoProgram;
use cairo_lang_casm::hints::PythonicHint;
use cairo_lang_sierra::program::Program as SierraProgram;
use cairo_lang_sierra_to_casm::{
    compiler::SierraToCasmConfig, metadata::calc_metadata_ap_change_only,
};
use cairo_lang_starknet_classes::compiler_version::current_compiler_version_id;
use cairo_lang_starknet_classes::NestedIntList;
use cairo_lang_utils::bigint::{deserialize_big_uint, serialize_big_uint, BigUintAsHex};
use cairo_vm::vm::errors::vm_errors::VirtualMachineError;
use itertools::Itertools;
use num_bigint::BigUint;
use num_integer::Integer;
use num_traits::Signed;
use serde::{Deserialize, Serialize};

fn skip_if_none<T>(opt_field: &Option<T>) -> bool {
    opt_field.is_none()
}

/// Represents a contract in the Starknet network.
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CasmProgramClass {
    #[serde(
        serialize_with = "serialize_big_uint",
        deserialize_with = "deserialize_big_uint"
    )]
    pub prime: BigUint,
    pub compiler_version: String,
    pub bytecode: Vec<BigUintAsHex>,
    #[serde(skip_serializing_if = "skip_if_none")]
    pub bytecode_segment_lengths: Option<NestedIntList>,

    // Optional pythonic hints in a format that can be executed by the python vm.
    pub pythonic_hints: Vec<(usize, Vec<String>)>,
    pub main: usize,
}

// Compile a Cairo 1 program
pub fn cairo_compile(sierra_program: &SierraProgram) -> Result<CasmProgramClass, Error> {
    let prime = Felt252::prime();
    let metadata = calc_metadata_ap_change_only(sierra_program)
        .map_err(|_| VirtualMachineError::Unexpected)?;
    let config = SierraToCasmConfig {
        gas_usage_check: false,
        max_bytecode_size: usize::MAX,
    };
    let cairo_program =
        cairo_lang_sierra_to_casm::compiler::compile(sierra_program, &metadata, config)?;

    println!(
        "{:#?}",
        cairo_program
            .instructions
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<String>>()
    );

    let AssembledCairoProgram { bytecode, hints } = cairo_program.assemble();
    let bytecode = bytecode
        .iter()
        .map(|big_int| {
            let (_q, reminder) = big_int.magnitude().div_rem(&prime);
            BigUintAsHex {
                value: if big_int.is_negative() {
                    &prime - reminder
                } else {
                    reminder
                },
            }
        })
        .collect_vec();

    let bytecode_segment_lengths =
        compute_bytecode_segment_lengths(sierra_program, &cairo_program, &bytecode).ok();

    let main_func = find_function(sierra_program, "::main")?;

    let pythonic_hints = hints
        .iter()
        .map(|(pc, hints)| {
            (
                *pc,
                hints
                    .iter()
                    .map(|hint| hint.get_pythonic_hint())
                    .collect_vec(),
            )
        })
        .collect_vec();

    let compiler_version = current_compiler_version_id().to_string();
    Ok(CasmProgramClass {
        prime,
        compiler_version,
        bytecode,
        bytecode_segment_lengths,
        pythonic_hints,
        main: main_func.entry_point.0,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use cairo_lang_compiler::{
        compile_prepared_db, db::RootDatabase, project::setup_project, CompilerConfig,
    };
    use cairo_lang_sierra::program::Program as SierraProgram;

    use super::cairo_compile;

    fn compile_to_sierra(filename: &str) -> SierraProgram {
        let compiler_config = CompilerConfig {
            replace_ids: true,
            ..CompilerConfig::default()
        };
        let mut db = RootDatabase::builder()
            .detect_corelib()
            .skip_auto_withdraw_gas()
            .build()
            .unwrap();
        let main_crate_ids = setup_project(&mut db, Path::new(filename)).unwrap();
        compile_prepared_db(&mut db, main_crate_ids, compiler_config).unwrap()
    }

    #[test]
    fn test() {
        let cairo_program_path: &str = "../cairo_programs/cairo-1-programs/nullable_dict.cairo";
        let sierra_program = compile_to_sierra(cairo_program_path);
        let casm_program_class = cairo_compile(&sierra_program).unwrap();

        println!(
            "{}",
            serde_json::to_string_pretty(&casm_program_class).unwrap()
        );
    }
}

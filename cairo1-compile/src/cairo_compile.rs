pub mod function;
pub mod segment;
pub mod type_resolver;

use crate::{
    cairo_compile::{
        function::find_function, segment::compute_bytecode_segment_lengths,
        type_resolver::TypeResolver,
    },
    error::Error,
};
use cairo_felt::Felt252;
use cairo_lang_casm::hints::PythonicHint;
use cairo_lang_casm::{assembler::AssembledCairoProgram, hints::Hint};
use cairo_lang_sierra::{
    extensions::{
        bitwise::BitwiseType, ec::EcOpType, gas::GasBuiltinType, pedersen::PedersenType,
        poseidon::PoseidonType, range_check::RangeCheckType, segment_arena::SegmentArenaType,
        starknet::syscalls::SystemType, NamedType,
    },
    ids::GenericTypeId,
    program::{FunctionSignature, Program as SierraProgram, StatementIdx},
};
use cairo_lang_sierra_to_casm::{
    compiler::SierraToCasmConfig, metadata::calc_metadata_ap_change_only,
};
use cairo_lang_starknet_classes::NestedIntList;
use cairo_lang_starknet_classes::{
    casm_contract_class::{
        CasmContractClass, CasmContractEntryPoint, StarknetSierraCompilationError,
    },
    compiler_version::current_compiler_version_id,
    contract_class::ContractEntryPoint,
};
use cairo_lang_utils::{
    bigint::{deserialize_big_uint, serialize_big_uint, BigUintAsHex},
    ordered_hash_map::OrderedHashMap,
    unordered_hash_set::UnorderedHashSet,
};
use cairo_vm::vm::errors::vm_errors::VirtualMachineError;
use convert_case::{Case, Casing};
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
    pub hints: Vec<(usize, Vec<Hint>)>,

    // Optional pythonic hints in a format that can be executed by the python vm.
    #[serde(skip_serializing_if = "skip_if_none")]
    pub pythonic_hints: Option<Vec<(usize, Vec<String>)>>,
    pub main: CasmProgramEntryPoint,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CasmProgramEntryPoint {
    /// The offset of the instruction that should be called within the contract bytecode.
    pub offset: usize,
    // list of builtins.
    pub builtins: Vec<String>,
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

    let builtin_types = UnorderedHashSet::<GenericTypeId>::from_iter([
        RangeCheckType::id(),
        BitwiseType::id(),
        PedersenType::id(),
        EcOpType::id(),
        PoseidonType::id(),
        SegmentArenaType::id(),
        GasBuiltinType::id(),
        SystemType::id(),
    ]);

    let as_casm_entry_point = |contract_entry_point: EntryPoint| {
        let Some(function) = sierra_program.funcs.get(contract_entry_point.function_idx) else {
            return Err(StarknetSierraCompilationError::EntryPointError);
        };
        let statement_id = function.entry_point;

        let (input_span, input_builtins) = contract_entry_point.signature.param_types.split_last().unwrap();

        println!("{:?}", input_builtins);
        println!("{:?}", sierra_program.type_declarations);

        let type_resolver = TypeResolver {
            type_decl: &sierra_program.type_declarations,
        };
        let (panic_result, output_builtins) = contract_entry_point.signature.ret_types.split_last().unwrap();

        for type_id in input_builtins.iter() {
            println!("{}", type_resolver.get_generic_id(type_id));
            // if !builtin_types.contains(type_resolver.get_generic_id(type_id)) {
            //     return Err(StarknetSierraCompilationError::InvalidBuiltinType(
            //         type_id.clone(),
            //     ));
            // }
        }
        let (system_ty, builtins) = input_builtins.split_last().unwrap();
        let (gas_ty, builtins) = builtins.split_last().unwrap();

        // Check that the last builtins are gas and system.
        if *type_resolver.get_generic_id(system_ty) != SystemType::id()
            || *type_resolver.get_generic_id(gas_ty) != GasBuiltinType::id()
        {
            return Err(
                StarknetSierraCompilationError::InvalidEntryPointSignatureWrongBuiltinsOrder,
            );
        }

        let builtins = builtins
            .iter()
            .map(|type_id| {
                type_resolver
                    .get_generic_id(type_id)
                    .0
                    .as_str()
                    .to_case(Case::Snake)
            })
            .collect_vec();

        let code_offset = cairo_program
            .debug_info
            .sierra_statement_info
            .get(statement_id.0)
            .ok_or(StarknetSierraCompilationError::EntryPointError)?
            .code_offset;

        Ok::<CasmProgramEntryPoint, StarknetSierraCompilationError>(CasmProgramEntryPoint {
            offset: code_offset,
            builtins,
        })
    };

    Ok(CasmProgramClass {
        prime,
        compiler_version,
        bytecode,
        bytecode_segment_lengths,
        hints,
        pythonic_hints: Some(pythonic_hints),
        main: as_casm_entry_point(EntryPoint {
            function_idx: main_func.entry_point.0,
            signature: &main_func.signature,
        })
        .unwrap(),
    })
}

pub struct EntryPoint<'a> {
    function_idx: usize,
    signature: &'a FunctionSignature,
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

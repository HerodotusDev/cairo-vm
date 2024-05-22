pub mod builtins;
pub mod function;
pub mod segment;

use crate::{
    cairo_compile::{
        builtins::{build_hints_dict, get_function_builtins},
        function::find_function,
    },
    error::Error,
};
use cairo_felt::Felt252;
use cairo_lang_sierra::{
    extensions::core::{CoreLibfunc, CoreType},
    program::Program as SierraProgram,
    program_registry::ProgramRegistry,
};
use cairo_lang_sierra_to_casm::{
    compiler::SierraToCasmConfig, metadata::calc_metadata_ap_change_only,
};
use cairo_lang_sierra_type_size::get_type_size_map;
use cairo_vm::{
    serde::deserialize_program::{ProgramJson, ReferenceManager},
    types::{program::Program, relocatable::MaybeRelocatable},
    vm::errors::{cairo_run_errors::CairoRunError, vm_errors::VirtualMachineError},
};
use std::collections::HashMap;

// Compile a Cairo 1 program
pub fn cairo_compile(sierra_program: &SierraProgram) -> Result<Program, Error> {
    let metadata = calc_metadata_ap_change_only(&sierra_program)?;
    let sierra_program_registry = ProgramRegistry::<CoreType, CoreLibfunc>::new(&sierra_program)?;
    let type_sizes = get_type_size_map(&sierra_program, &sierra_program_registry).unwrap();
    let config = SierraToCasmConfig {
        gas_usage_check: false,
        max_bytecode_size: usize::MAX,
    };
    let cairo_program =
        cairo_lang_sierra_to_casm::compiler::compile(sierra_program, &metadata, config)?;

    let data: Vec<MaybeRelocatable> = cairo_program
        .instructions
        .iter()
        .flat_map(|inst| inst.assemble().encode())
        .map(|x| cairo_vm::Felt252::from(&x))
        .map(MaybeRelocatable::from)
        .collect();
    let hints_dict = build_hints_dict(cairo_program.instructions.iter());

    let main_func = find_function(sierra_program, "::main")?;
    let signature = &main_func.signature;
    // The builtins in the formatting expected by the runner.
    let (builtins, _) = get_function_builtins(&signature.param_types);

    Ok(Program::new(
        builtins,
        data,
        Some(0),
        hints_dict,
        ReferenceManager {
            references: Vec::new(),
        },
        HashMap::new(),
        vec![],
        None,
    )?)
}

#[cfg(test)]
mod tests {
    use super::cairo_compile;
    use cairo_lang_compiler::{
        compile_prepared_db, db::RootDatabase, project::setup_project, CompilerConfig,
    };
    use cairo_lang_sierra::program::Program as SierraProgram;
    use cairo_vm::serde::{deserialize_program::ProgramJson, serialize_program::ProgramSerializer};
    use std::path::Path;

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
        let cairo_program_path: &str = "../cairo_programs/cairo-1-programs/factorial.cairo";
        let sierra_program = compile_to_sierra(cairo_program_path);
        let program = cairo_compile(&sierra_program).unwrap();
        let program_json = ProgramJson::from(ProgramSerializer::from(&program));
        println!("{}", serde_json::to_string(&program_json).unwrap());
    }
}

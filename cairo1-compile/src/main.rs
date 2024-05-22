use cairo1_compile::{cairo_compile::cairo_compile, error::Error};
use cairo_lang_compiler::{
    compile_prepared_db, db::RootDatabase, project::setup_project, CompilerConfig,
};
use cairo_vm::serde::{deserialize_program::ProgramJson, serialize_program::ProgramSerializer};
use clap::{Parser, ValueHint};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(value_parser, value_hint=ValueHint::FilePath, required=true)]
    filename: PathBuf,
    #[clap(value_parser, value_hint=ValueHint::FilePath, required=true)]
    outfile: PathBuf,
}

fn run(args: impl Iterator<Item = String>) -> Result<(), Error> {
    let args = Args::try_parse_from(args)?;

    // Try to parse the file as a sierra program
    let file = std::fs::read(&args.filename)?;
    let sierra_program = match serde_json::from_slice(&file) {
        Ok(program) => program,
        Err(_) => {
            // If it fails, try to compile it as a cairo program
            let compiler_config = CompilerConfig {
                replace_ids: true,
                ..CompilerConfig::default()
            };
            let mut db = RootDatabase::builder()
                .detect_corelib()
                .skip_auto_withdraw_gas()
                .build()
                .unwrap();
            let main_crate_ids = setup_project(&mut db, &args.filename).unwrap();
            compile_prepared_db(&mut db, main_crate_ids, compiler_config).unwrap()
        }
    };

    let program = cairo_compile(&sierra_program).unwrap();
    let program_json = ProgramJson::from(ProgramSerializer::from(&program));
    std::fs::write(
        args.outfile.as_path(),
        format!("{}", serde_json::to_string(&program_json).unwrap()),
    )?;

    Ok(())
}

fn main() -> Result<(), Error> {
    match run(std::env::args()) {
        Err(Error::Cli(err)) => err.exit(),
        Ok(_) => Ok(()),
        Err(err) => panic!("{err}"),
    }
}

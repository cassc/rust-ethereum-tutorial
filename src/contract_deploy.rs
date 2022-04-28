use std::path::PathBuf;

use clap::Parser;
use eyre::{eyre, ContextCompat};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Directory holding our contracts
    #[clap(short, long)]
    contract_root: String,
}

use ethers::prelude::{ConfigurableArtifacts, Project, ProjectCompileOutput, ProjectPathsConfig};
use eyre::Result;
use tracing::{info, instrument, Level};

fn enable_tracing() -> Result<()> {
    let collector = tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(collector)?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    enable_tracing()?;

    let args = Args::parse();
    let root: String = args.contract_root;

    let project = compile(&root).await?;

    println!("Project compilation success: {project:?}");

    print_project(project).await?;

    Ok(())
}

#[instrument]
pub async fn compile(root: &str) -> Result<ProjectCompileOutput<ConfigurableArtifacts>> {
    // soldity project root
    let root = PathBuf::from(root);
    if !root.exists() {
        return Err(eyre!("Project root {root:?} does not exists!"));
    }

    let paths = ProjectPathsConfig::builder()
        .root(&root)
        .sources(&root)
        .build()?;

    // get the solc project instance using the paths above
    let project = Project::builder()
        .paths(paths)
        .ephemeral()
        .no_artifacts()
        .build()?;

    // the async wrapper is needed to to make a blocking client async
    // See discussion here https://githubhot.com/index.php/repo/seanmonstar/reqwest/issues/1450
    let output = async { project.compile() }.await?;
    if output.has_compiler_errors() {
        Err(eyre!(
            "Compiling solidity project failed: {:?}",
            output.output().errors
        ))
    } else {
        Ok(output.clone())
    }
}

pub async fn print_project(project: ProjectCompileOutput<ConfigurableArtifacts>) -> Result<()> {
    let artifacts = project.into_artifacts();
    let mut contracts = Vec::new();
    for (id, artifact) in artifacts {
        let name = id.name;
        let abi = artifact.abi.context("No ABI found for artificat {name}")?;

        println!("CONTRACT: {:?}", name);
        contracts.push(name);

        let contract = &abi.abi;
        let functions = contract.functions();
        let functions = functions.cloned();
        let constructor = contract.constructor();

        if let Some(constructor) = constructor {
            let args = &constructor.inputs;
            println!("CONSTRUCTOR args: {args:?}");
        }

        for func in functions {
            let name = &func.name;
            let params = &func.inputs;
            println!("FUNCTION {name} {params:?}");
        }
    }
    Ok(())
}

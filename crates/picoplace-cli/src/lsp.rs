use clap::Args;

#[derive(Args)]
pub struct LspArgs {}

pub fn execute(_args: LspArgs) -> anyhow::Result<()> {
    picoplace_lang::lsp_with_eager(true)?;
    Ok(())
}

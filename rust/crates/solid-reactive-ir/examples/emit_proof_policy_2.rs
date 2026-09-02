use std::io::{self, Write as _};

use solid_reactive_ir::contract_semantics::certification::proof_policy_2;

fn main() -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(proof_policy_2().audit_manifest())?;
    stdout.write_all(b"\n")
}

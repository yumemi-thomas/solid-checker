//! Research consumer of the existing dialect adapter and normalized fact seam.
//! No compiler producer types cross this boundary.
use serde_json::{Value, json};
use solid_facts::compiler::{AnalysisRequest, CompilerOptions};
use solid_v2_compiler::analyze_with_materialized_output;
use std::io::{self, Read};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let cases: Vec<Value> = serde_json::from_str(&input)?;
    let mut results = Vec::with_capacity(cases.len());
    for case in cases {
        let path = case["path"].as_str().ok_or("missing path")?;
        let source = std::fs::read_to_string(path)?;
        let options: CompilerOptions = serde_json::from_value(case["options"].clone())?;
        let request = AnalysisRequest::new(path, source, options);
        let result = match analyze_with_materialized_output(&request) {
            Ok(compilation) => {
                compilation.execution_map.validate(&request.source)?;
                json!({
                    "status": "facts",
                    "path": path,
                    "options": request.compiler_options,
                    "executionMap": compilation.execution_map,
                    "output": compilation.output,
                    "sourceMap": compilation.source_map,
                    "traceOnOffOutputIdentity": true,
                    "authority": false,
                    "certification": false,
                })
            }
            Err(error) => json!({
                "status": "refused",
                "path": path,
                "options": request.compiler_options,
                "error": error.to_string(),
                "authority": false,
                "certification": false,
            }),
        };
        results.push(result);
    }
    println!("{}", serde_json::to_string(&results)?);
    Ok(())
}

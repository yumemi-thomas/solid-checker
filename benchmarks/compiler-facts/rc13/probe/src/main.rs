use serde::Deserialize;
use serde_json::{Value, json};
use solidjs_compiler::{CompileOptions, Generate, SourceNames, compile};
use std::io::{self, Read};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Options {
    generate: String,
    module_name: String,
    built_ins: Vec<String>,
    static_marker: String,
    hydratable: bool,
    dev: bool,
    source_map: bool,
    server_components: bool,
    hoist_props: bool,
    components: bool,
    bindings: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: String,
    source: String,
    options: Options,
}

fn outcome(source: &str, options: &CompileOptions) -> Value {
    match compile(source, options) {
        Ok(output) => json!({"status":"compiled", "code":output.code, "map":output.source_map,
            "css":output.css,"cssHash":output.css_hash}),
        Err(error) => {
            json!({"status":"refused","kind":format!("{:?}",error.kind()),"error":error.to_string()})
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let requests: Vec<Request> = serde_json::from_str(&input)?;
    let mut rows = Vec::new();
    for request in requests {
        let o = request.options;
        let options = CompileOptions {
            filename: Some("/compiler-rebase/input.tsx".into()),
            generate: match o.generate.as_str() {
                "dom" => Generate::Dom,
                "ssr" => Generate::Ssr,
                "universal" => Generate::Universal,
                "dynamic" => Generate::Dynamic,
                _ => return Err("unsupported request mode".into()),
            },
            module_name: o.module_name,
            built_ins: o.built_ins,
            static_marker: o.static_marker,
            hydratable: o.hydratable,
            dev: o.dev,
            source_map: o.source_map,
            server_components: o.server_components,
            hoist_props: o.hoist_props,
            source_names: SourceNames {
                components: o.components,
                bindings: o.bindings,
            },
            ..CompileOptions::default()
        };
        let plain = outcome(&request.source, &options);
        let mut row = json!({"id":request.id,"plain":plain});
        #[cfg(feature = "trace")]
        {
            let traced_options = CompileOptions {
                semantic_trace: true,
                ..options
            };
            row["traced"] = outcome(&request.source, &traced_options);
            if let Ok(output) = compile(&request.source, &traced_options) {
                row["trace"] = serde_json::to_value(output.semantic_trace)?;
            }
        }
        #[cfg(not(feature = "trace"))]
        {
            row["traced"] = Value::Null;
        }
        rows.push(row);
    }
    println!("{}", serde_json::to_string(&rows)?);
    Ok(())
}

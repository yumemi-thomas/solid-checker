use super::*;

fn app(source: &str) -> Option<String> {
    refusal(
        &serde_json::json!({"build": source}),
        ManifestRole::Application,
    )
}

#[test]
fn vite_free_scripts_are_premised() {
    for source in [
        "tsc -b",
        "eslint .",
        "playwright test",
        "vitest",
        "vp build",
        "node scripts/build.mjs",
        "npm run i18n && tsr generate",
        "rm -rf dist && echo 'ok'",
        "pnpm add @vitejs/plugin-react",
    ] {
        assert!(app(source).is_none(), "{source}");
        assert!(
            refusal(
                &serde_json::json!({"build": source}),
                ManifestRole::Enclosing
            )
            .is_none(),
            "{source}"
        );
    }
}

#[test]
fn exact_vite_commands_only() {
    for source in [
        " vite build --outDir dist ",
        "vite",
        "vite dev",
        "vite serve",
        "vite preview",
        "vite build --port 3000 --host localhost --open --strictPort --base /app/ --outDir=dist --emptyOutDir --sourcemap --minify terser --logLevel warn --clearScreen false --force --cors --watch",
        "vite --host --minify",
        "tsc -b && vite build",
        "npm run i18n && tsr generate && tsc -b && vite build",
        "vite build&&tsc -p tsconfig.build.json",
        "cdk synth && vite build",
    ] {
        assert!(app(source).is_none(), "{source}");
    }
    for source in [
        "vite build --config other.ts",
        "vite --mode test",
        "vite --root .",
        "vite .",
        "vite --configLoader native",
        "vite --unknown",
        "vite --port nope",
        "vite --outDir --config",
        "vite --open=foo",
        "VITE build",
        "VITE_MODE=x vite",
        "vite build\n",
        "vite build *.ts",
        "vite build $VITE_FLAGS",
        "vite build $(cat flags)",
        "\"vite\" build",
        "v\\ite build",
        "tsc && vite build --mode staging",
        "cd .. && vite build",
        "cd .. || cd app && vite build",
        "X=1 cd .. && vite build",
        "command cd ..; vite build",
        "builtin pushd ..; vite build",
        "true;cd ..;vite build",
        "cd;vite build",
        "vite build; tsc -p tsconfig.build.json",
        "tsc | vite build",
        "{cd,..};vite build",
        "{cd,..} && vite build",
        "c[d] .. && vite build",
        "c* .. && vite build",
        "{cd ..;};vite build",
        "chdir .. && vite build",
        "builtin chdir .. && vite build",
        "CD .. && vite build",
        "cd.. && vite build",
        "cd/d .. && vite build",
        "PATH=./bin && vite build",
        "export PATH=./bin && vite build",
        "alias vite=other && vite build",
        "@cd .. && vite build",
        "@chdir .. && vite build",
        "@pushd .. && vite build",
        "D: && vite build",
        "path C:/alternate/bin && vite build",
        "set /a PATH-=1 && vite build",
        "v^ite build --config ../other.ts",
        "vi^te build --mode staging",
        "env -Svite build --config ../other.ts",
        "cd .. && env -Svite build",
        "cmd /cvite build --config ../other.ts",
        "node_modules\\.bin\\vite.cmd build --config ../other.ts",
        "node node_modules\\vite\\bin\\vite.js build --config ../other.ts",
        "path.. && vite build",
        "node node_modules\\vite\\dist\\node\\cli.js build --config ../other.ts",
        "path/alternate/bin && vite build",
        "v^\nite build --config ../other.ts",
        "c^d ..&vite build",
        "! true && vite build",
        "# x && vite build",
        "true \u{b}cd .. && vite build",
        "&& vite build",
        "vite build &&",
        "true\nvite build --config ../other.ts",
        "v\\\nite build --config ../other.ts",
        "npx vite build",
        "pnpm --dir ../.. exec vite build",
        "pnpm -C.. exec vite dev",
        "node node_modules/vite/bin/vite.js build",
        "sh -c 'vite build'",
        "sh -c 'vit\"e\" build --config ../other.ts'",
        "electron-vite build",
        "env X=1 vite",
    ] {
        assert!(app(source).unwrap().contains("scripts.build"), "{source}");
    }
    assert!(
        refusal(
            &serde_json::json!({"build": ["vite"]}),
            ManifestRole::Application
        )
        .unwrap()
        .contains("non-literal")
    );
}

#[test]
fn enclosing_manifests_refuse_vite() {
    for source in [
        "vite",
        "vite build",
        "tsc && vite build",
        "pnpm --filter app exec vite",
        "v\\\nite build --config ../other.ts",
        "true\nvite build --config ../other.ts",
        "v^ite build --config ../other.ts",
        "env -Svite build --config ../other.ts",
        "node_modules\\.bin\\vite.cmd build --config ../other.ts",
        "v^\nite build --config ../other.ts",
        "node node_modules\\vite\\dist\\node\\cli.js build --config ../other.ts",
    ] {
        assert_eq!(
            refusal(
                &serde_json::json!({"postbuild": source}),
                ManifestRole::Enclosing
            )
            .unwrap(),
            "scripts.postbuild: enclosing manifest launches Vite from a different directory"
        );
    }
}

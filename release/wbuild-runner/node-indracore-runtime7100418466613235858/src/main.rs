
				use substrate_wasm_builder::build_project_with_default_rustflags;

				fn main() {
					build_project_with_default_rustflags(
						"/home/ayoung/project/indracore/target/release/build/node-indracore-runtime-7e014d961c147dcf/out/wasm_binary.rs",
						"/home/ayoung/project/indracore/node/runtime/Cargo.toml",
						"-Clink-arg=--export=__heap_base -C link-arg=--import-memory ",
					)
				}
			
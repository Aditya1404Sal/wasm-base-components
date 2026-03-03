pub mod bindings {
    wit_bindgen::generate!({ generate_all });
}

use bindings::exports::betty_blocks::actions::actions::{Error, Guest, RunInput, RunOutput};

struct Component;

impl Guest for Component {
    fn call(input: RunInput) -> Result<RunOutput, Error> {
        let _ = input;
        Ok(RunOutput {
            result: r#"{"output": "mock action executed successfully"}"#.to_string(),
        })
    }
}

bindings::export!(Component with_types_in bindings);

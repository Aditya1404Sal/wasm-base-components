use wasmcloud_component::wasi::io::streams;
use wasmcloud_component::{
    http::{self, IncomingBody},
    wasi::io::{poll::Pollable, streams::InputStream},
};

pub mod bindings {
    wit_bindgen::generate!({ generate_all });
}

use crate::bindings::betty_blocks::types::actions::{Input, Output, Payload, call, health};

struct Component;

#[derive(serde::Deserialize, Debug)]
#[cfg_attr(test, derive(serde::Serialize))]
struct PayloadWrapper {
    input: String,
    configurations: String,
}
#[derive(serde::Deserialize, Debug)]
#[cfg_attr(test, derive(serde::Serialize))]
struct InputWrapper {
    action_id: String,
    payload: PayloadWrapper,
}

impl From<InputWrapper> for Input {
    fn from(val: InputWrapper) -> Self {
        Input {
            action_id: val.action_id,
            payload: Payload {
                input: val.payload.input,
                configurations: val.payload.configurations,
            },
        }
    }
}

// 2**24 = 16mb
const MAX_READ_MB: u64 = 16;
const MEGABYTE: u64 = 2u64.pow(20);
const MAX_READ: u64 = MAX_READ_MB * MEGABYTE;

trait IncomingRequestImpl {
    fn method(&self) -> &http::Method;
    fn body(&self) -> &impl IncomingBodyImpl;
}

trait IncomingBodyImpl {
    fn subscribe(&self) -> impl PollableImpl;
    fn read(&self, max_amount: u64) -> Result<Vec<u8>, streams::StreamError>;
}

impl IncomingRequestImpl for http::IncomingRequest {
    fn method(&self) -> &http::Method {
        http::IncomingRequest::method(self)
    }

    fn body(&self) -> &impl IncomingBodyImpl {
        http::IncomingRequest::body(self)
    }
}

impl IncomingBodyImpl for IncomingBody {
    fn subscribe(&self) -> impl PollableImpl {
        InputStream::subscribe(self)
    }
    fn read(&self, max_amount: u64) -> Result<Vec<u8>, streams::StreamError> {
        InputStream::read(self, max_amount)
    }
}

trait PollableImpl {
    fn block(&self);
}

impl PollableImpl for Pollable {
    fn block(&self) {
        Pollable::block(self)
    }
}

#[derive(Debug, PartialEq)]
enum Error {
    InvalidBody(String),
    InvalidInput(String),
    InputTooLarge(u64),
    FailedToReadBody(String),
    ActionCallFailed(String),
    HealthCheckFailed(String),
}

impl From<Error> for http::Response<String> {
    fn from(val: Error) -> Self {
        match val {
            Error::InvalidBody(message) => {
                http::Response::builder().status(500).body(message).unwrap()
            }
            Error::InvalidInput(message) => {
                http::Response::builder().status(400).body(message).unwrap()
            }
            Error::InputTooLarge(amount_in_bytes) => http::Response::builder()
                .status(400)
                .body(format!(
                    "Body size exceeded the maximum {:.3}mb > {}mb",
                    (amount_in_bytes as f32) / (MEGABYTE as f32),
                    MAX_READ_MB
                ))
                .unwrap(),
            Error::FailedToReadBody(message) => {
                http::Response::builder().status(500).body(message).unwrap()
            }
            Error::ActionCallFailed(message) => {
                http::Response::builder().status(400).body(message).unwrap()
            }
            Error::HealthCheckFailed(message) => {
                http::Response::builder().status(400).body(message).unwrap()
            }
        }
    }
}

fn inner_handle<F>(
    request: impl IncomingRequestImpl,
    call_function: F,
) -> Result<http::Response<String>, Error>
where
    F: for<'a> FnOnce(&'a Input) -> Result<Output, String>,
{
    // Use GET for health checks because cant define multiple paths in wadm in kubernetes
    if request.method() == http::Method::GET {
        let health_status = health().map_err(Error::HealthCheckFailed)?;
        return Ok(http::Response::new(health_status));
    }

    let body = request.body();

    body.subscribe().block();

    // maybe we can use read_to_end, but that doesn't have a max size
    let mut body_bytes = Vec::new();
    loop {
        let chunk = body
            .read(MAX_READ)
            .map_err(|e| Error::FailedToReadBody(e.to_string()))?;
        if chunk.is_empty() {
            break;
        }
        if (body_bytes.len() as u64) > MAX_READ {
            return Err(Error::InputTooLarge(body_bytes.len() as u64));
        }
        body_bytes.extend_from_slice(&chunk);
    }

    let input_wrapper = serde_json::from_slice::<InputWrapper>(&body_bytes).map_err(|e| {
        if e.is_eof() || e.is_io() {
            Error::InvalidBody(e.to_string())
        } else {
            Error::InvalidInput(e.to_string())
        }
    })?;

    let input = input_wrapper.into();
    let result = call_function(&input).map_err(Error::ActionCallFailed)?;

    Ok(http::Response::new(result.result))
}

impl http::Server for Component {
    fn handle(
        request: http::IncomingRequest,
    ) -> http::Result<http::Response<impl http::OutgoingBody>> {
        match inner_handle(request, call) {
            Ok(response) => Ok(response),
            Err(e) => Ok(e.into()),
        }
    }
}

http::export!(Component);

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use wasmcloud_component::http::StatusCode;

    use super::*;

    struct TestIncomingRequest {
        body: TestIncomingRequestBody,
    }

    struct TestIncomingRequestBody {
        data: Vec<u8>,
        already_read: AtomicUsize,
        chunk_size: usize,
    }

    struct TestPollable {}

    impl IncomingRequestImpl for TestIncomingRequest {
        fn method(&self) -> &http::Method {
            &http::Method::POST
        }

        fn body(&self) -> &impl IncomingBodyImpl {
            &self.body
        }
    }

    impl IncomingBodyImpl for TestIncomingRequestBody {
        fn subscribe(&self) -> impl PollableImpl {
            TestPollable {}
        }

        fn read(&self, _: u64) -> Result<Vec<u8>, streams::StreamError> {
            let from = self
                .already_read
                .fetch_add(self.chunk_size, std::sync::atomic::Ordering::Relaxed);
            let until = (from + self.chunk_size).min(self.data.len());
            let chunk = if let Some(chunk) = self.data.get(from..until) {
                chunk.to_vec()
            } else {
                Vec::new()
            };
            Ok(chunk)
        }
    }

    impl PollableImpl for TestPollable {
        fn block(&self) {}
    }

    #[test]
    fn handles_loading_the_input_in_chunks() {
        let input = InputWrapper {
            action_id: String::from("951e9a1360bc44d8a28943ab94d461be"),
            payload: PayloadWrapper {
                input: serde_json::Value::Object(Default::default()).to_string(),
                configurations: serde_json::Value::Object(Default::default()).to_string(),
            },
        };

        let test_action = |action_input: &Input| -> Result<Output, String> {
            assert_eq!(action_input.action_id, input.action_id);
            assert_eq!(action_input.payload.input, input.payload.input);
            assert_eq!(
                action_input.payload.configurations,
                input.payload.configurations
            );
            Ok(Output {
                result: String::from("done"),
            })
        };

        let request = TestIncomingRequest {
            body: TestIncomingRequestBody {
                data: serde_json::to_vec(&input).unwrap(),
                already_read: AtomicUsize::new(0),
                chunk_size: 2,
            },
        };
        let response = inner_handle(request, test_action).unwrap();
        assert_eq!(response.body(), "done");
    }

    #[test]
    fn maximum_input_is_reached() {
        let object = serde_json::json!(
            {
                "abc": "abcdefgh",
                "def": "abcdefgh",
            }
        );

        let mut data = Vec::new();

        for _ in 0..390000 {
            data.push(object.clone());
        }

        let input = serde_json::json!(
            {
                "data": data
            }
        );

        let input = InputWrapper {
            action_id: String::from("951e9a1360bc44d8a28943ab94d461be"),
            payload: PayloadWrapper {
                input: input.to_string(),
                configurations: serde_json::Value::Object(Default::default()).to_string(),
            },
        };
        assert!(serde_json::to_vec(&input).unwrap().len() as u64 > MAX_READ);

        let test_action = |action_input: &Input| -> Result<Output, String> {
            assert_eq!(action_input.action_id, input.action_id);
            assert_eq!(action_input.payload.input, input.payload.input);
            assert_eq!(
                action_input.payload.configurations,
                input.payload.configurations
            );
            Ok(Output {
                result: String::from("done"),
            })
        };

        let request = TestIncomingRequest {
            body: TestIncomingRequestBody {
                data: serde_json::to_vec(&input).unwrap(),
                already_read: AtomicUsize::new(0),
                chunk_size: 4096,
            },
        };
        let response = inner_handle(request, test_action).unwrap_err();
        assert_eq!(response, Error::InputTooLarge(16781312));
    }

    #[test]
    fn test_error_messages() {
        let response = http::Response::from(Error::InvalidBody("it broke :(".to_string()));
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.body(), "it broke :(");

        let response = http::Response::from(Error::InvalidInput("it broke :(".to_string()));
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.body(), "it broke :(");

        let response = http::Response::from(Error::FailedToReadBody("it broke :(".to_string()));
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.body(), "it broke :(");

        let response = http::Response::from(Error::InputTooLarge(16781312));
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.body(),
            "Body size exceeded the maximum 16.004mb > 16mb"
        );

        let response = http::Response::from(Error::ActionCallFailed("it broke :(".to_string()));
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.body(), "it broke :(");

        let response = http::Response::from(Error::HealthCheckFailed("it broke :(".to_string()));
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.body(), "it broke :(");
    }
}

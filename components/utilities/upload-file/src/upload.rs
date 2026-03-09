use anyhow::Result;
use multipart::client::lazy::Multipart;
use std::io::{Cursor, Read};
use tracing::{debug, error};
use wstd::{
    http::{body::BodyForthcoming, Client, Request},
    io::AsyncWrite,
};

use crate::bindings::{
    betty_blocks::data_api::data_api_utilities::{self, Model, PresignedPost, Property},
    exports::betty_blocks::file::upload_file::UploadResult,
};

const NETWORK_BUF_SIZE: usize = 64 * 1024; // 64kb

pub async fn upload_bytes_internal(
    model: Model,
    property: Property,
    file_bytes: Vec<u8>,
    filename: String,
    content_type: String,
) -> Result<UploadResult> {
    let file_size = file_bytes.len() as u64;

    let presigned_post =
        data_api_utilities::fetch_presigned_post(&model, &property, &content_type,&filename)
            .map_err(|e| {
                error!(
                    "upload_bytes_internal: Failed to fetch presigned URL: {}",
                    e
                );
                anyhow::anyhow!("Failed to fetch presigned URL: {}", e)
            })?;

    upload_to_presigned_post(&presigned_post, file_bytes, &filename, &content_type).await?;

    Ok(UploadResult {
        reference: presigned_post.reference,
        file_size,
        message: Some("Upload successful".into()),
    })
}

async fn upload_to_presigned_post(
    presigned_post: &PresignedPost,
    file_bytes: Vec<u8>,
    filename: &str,
    content_type: &str,
) -> Result<()> {
    let client = Client::new();
    let mut form = Multipart::new();

    for field in &presigned_post.fields {
        form.add_text(field.key.clone(), field.value.clone());
    }

    let cursor = Cursor::new(file_bytes);

    let mime: mime::Mime = content_type
        .parse()
        .unwrap_or(mime::APPLICATION_OCTET_STREAM);

    form.add_stream("file", cursor, Some(filename), Some(mime));

    let mut prepared = form
        .prepare()
        .map_err(|e| anyhow::anyhow!("Failed to prepare multipart form: {e}"))?;

    let content_type_header = format!("multipart/form-data; boundary={}", prepared.boundary());

    let request = Request::post(&presigned_post.url)
        .header("content-type", &*content_type_header)
        .body(BodyForthcoming)
        .map_err(|e| anyhow::anyhow!("Failed to build upload request: {e}"))?;

    let (mut outgoing_body, response_future) = client
        .start_request(request)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start upload request: {e}"))?;

    let mut buf = [0u8; NETWORK_BUF_SIZE];
    loop {
        let n = prepared
            .read(&mut buf)
            .map_err(|e| anyhow::anyhow!("Failed to read multipart body: {e}"))?;
        if n == 0 {
            break;
        }
        outgoing_body
            .write_all(&buf[..n])
            .await
            .map_err(|e| anyhow::anyhow!("Failed to write to outgoing body: {e}"))?;
    }

    Client::finish(outgoing_body, None)
        .map_err(|e| anyhow::anyhow!("Failed to finish outgoing body: {e}"))?;

    let response = response_future
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get upload response: {e}"))?;

    let status = response.status().as_u16();
    debug!("Status: {}", status);

    if status >= 300 {
        let mut err_body = response.into_body();
        let err = match err_body.bytes().await {
            Ok(b) => String::from_utf8_lossy(&b).to_string(),
            Err(_) => String::new(),
        };
        debug!("Error body: {}", err);
        return Err(anyhow::anyhow!(
            "upload failed with status {}: {}",
            status,
            err
        ));
    }

    debug!("Presigned POST upload succeeded");
    Ok(())
}

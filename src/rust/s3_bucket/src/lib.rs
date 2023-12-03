#[allow(unused_imports)]
use anyhow::{anyhow, bail, Context, Error, Result};

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

pub use rusoto_core::{request::HttpClient, ByteStream, RusotoError};

// use common_macros::let_from_env;
pub use rusoto_core::credential::StaticProvider;
pub use rusoto_core::Region;

use rusoto_s3::{
    DeleteObjectRequest,
    GetObjectRequest,
    HeadObjectRequest,
    // ListObjectsV2Output,
    ListObjectsV2Request,
    PutObjectRequest,
    S3Client,
    StreamingBody,
    S3,
};

use futures::stream::{self, Stream, StreamExt};

use futures::TryStreamExt;
use tokio::io::AsyncRead;
use tokio_util::codec;

use bytes::Bytes;

pub struct HeadResponse {
    pub content_type: Option<String>,
    pub content_length: Option<i64>,
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Clone)]
pub struct S3Bucket {
    client: S3Client,
    bucket: String,
    // fetch_limit: usize,
    // max_attempt: usize,
}

pub struct S3BucketBuilder {
    provider: Option<StaticProvider>,
    region: Option<Region>,
    bucket: String,
}

impl S3BucketBuilder {
    pub fn new(bucket: String) -> Self {
        Self {
            bucket,
            provider: None,
            region: None,
        }
    }
    pub fn region(self, region: Region) -> Self {
        Self {
            bucket: self.bucket,
            region: Some(region),
            provider: self.provider,
        }
    }
    pub fn provider(self, provider: StaticProvider) -> Self {
        Self {
            bucket: self.bucket,
            region: self.region,
            provider: Some(provider),
        }
    }
    pub fn build(self) -> Result<S3Bucket> {
        let request_dispatcher = HttpClient::new()?;
        let provider = match self.provider {
            Some(provider) => provider,
            None => Self::default_provider()?,
        };
        let region = self.region.unwrap_or_else(Self::default_region);
        let client = S3Client::new_with(request_dispatcher, provider, region);
        Ok(S3Bucket::new(client, self.bucket))
    }
    fn default_region() -> Region {
        let name = std::env::var("S3_REGION_NAME").unwrap_or_else(|_| "us-east-1".to_owned());
        let endpoint = std::env::var("S3_REGION_ENDPOINT")
            .unwrap_or_else(|_| "https://storage.yandexcloud.net".to_owned());

        Region::Custom { name, endpoint }
    }
    fn default_provider() -> Result<StaticProvider> {
        todo!();
        // let_from_env!(access_key, S3_ACCESS_KEY);
        // let_from_env!(secret_access_key, S3_SECRET_ACCESS_KEY);
        // Ok(StaticProvider::new(
        //     access_key,
        //     secret_access_key,
        //     None,
        //     None,
        // ))
    }
}

impl S3Bucket {
    fn new(client: S3Client, bucket: String) -> Self {
        Self {
            client,
            bucket,
            // fetch_limit: config.content.s3.fetch_limit,
            // max_attempt: config.content.s3.max_attempt,
        }
    }
    pub async fn head(&self, key: String) -> Result<Option<HeadResponse>> {
        let key_clone = key.to_owned();
        // https://rusoto.github.io/rusoto/rusoto_s3/struct.HeadObjectRequest.html
        let req = HeadObjectRequest {
            bucket: self.bucket.clone(),
            key: key.clone(),
            ..Default::default()
        };
        match self.client.head_object(req).await {
            Ok(resp) => {
                if let Some(delete_marker) = resp.delete_marker {
                    if delete_marker {
                        warn!("got delete marker for {}", key);
                        return Ok(None);
                    }
                }
                let content_length = resp.content_length.filter(|&i| i >= 0);
                //     match resp.content_length {
                //     None => None,
                //     Some(i) => {
                //         if i < 0 {
                //             None
                //         } else {
                //             Some(i)
                //         }
                //     }
                // };
                let mut to_be_parsed = resp
                    .last_modified
                    .as_ref()
                    .unwrap()
                    .to_owned()
                    .strip_suffix("GMT")
                    .unwrap()
                    .to_owned();
                to_be_parsed.push_str("+0000");
                let last_modified = match resp.last_modified.as_ref() {
                    None => None,
                    Some(s) => {
                        let mut to_be_parsed = s
                            .strip_suffix("GMT")
                            .ok_or_else(|| anyhow!("failed to strip GMT from {}", s))?
                            .to_owned();
                        to_be_parsed.push_str("+0000");
                        let parsed = chrono::DateTime::parse_from_str(
                            &to_be_parsed,
                            "%a, %d %b %Y %H:%M:%S %z",
                        )?
                        .into();
                        Some(parsed)
                    }
                };
                info!("{:?}", last_modified);
                Ok(Some(HeadResponse {
                    content_type: resp.content_type,
                    content_length,
                    last_modified,
                }))
            }
            Err(rusoto_error) => match rusoto_error {
                RusotoError::Service(_) => Ok(None),
                RusotoError::Unknown(resp) => {
                    if resp.status == 404 {
                        Ok(None)
                    } else {
                        error!("exists {}: {:?}", key_clone, resp);
                        bail!("{:?}", resp);
                    }
                }
                _ => {
                    error!("exists {}: {:?}", key_clone, rusoto_error);
                    bail!(rusoto_error)
                }
            },
        }
    }
    pub async fn delete(&self, key: String) -> Result<()> {
        // https://rusoto.github.io/rusoto/rusoto_s3/struct.DeleteObjectRequest.html
        let req = DeleteObjectRequest {
            bucket: self.bucket.clone(),
            key,
            ..Default::default()
        };
        let _ = self.client.delete_object(req).await?;
        Ok(())
    }
    pub async fn download(&self, key: String) -> Result<Option<StreamingBody>> {
        // https://rusoto.github.io/rusoto/rusoto_s3/struct.GetObjectRequest.html
        let req = GetObjectRequest {
            bucket: self.bucket.clone(),
            key,
            ..Default::default()
        };
        let resp = self.client.get_object(req).await?;
        Ok(resp.body)
    }
    pub async fn upload(&self, key: String, object_to_upload: ObjectToUpload) -> Result<()> {
        // https://rusoto.github.io/rusoto/rusoto_s3/struct.PutObjectRequest.html
        let req = PutObjectRequest {
            body: Some(object_to_upload.body),
            content_length: object_to_upload.content_length,
            content_type: object_to_upload.content_type,
            bucket: self.bucket.clone(),
            key,
            ..Default::default()
        };
        let _ = self.client.put_object(req).await?;
        Ok(())
    }
    pub async fn list(&self, arg: ListArg) -> Result<ListRet> {
        let list_request = ListObjectsV2Request {
            bucket: self.bucket.clone(),
            max_keys: arg.max_keys,
            continuation_token: arg.continuation_token,
            prefix: arg.prefix,
            ..Default::default()
        };
        let resp = self.client.list_objects_v2(list_request).await?;
        trace!("resp: {:?}", resp);
        if let Some(items) = resp.contents {
            let mut list_items: Vec<ListItem> = vec![];
            for item in items {
                #[allow(clippy::manual_map)]
                let last_modified = match item.last_modified {
                    None => None,
                    Some(s) => Some(chrono::DateTime::parse_from_rfc3339(&s)?.into()),
                };
                let list_item = ListItem {
                    key: item.key,
                    size: item.size,
                    storage_class: item.storage_class,
                    last_modified,
                };
                list_items.push(list_item);
            }
            if resp.is_truncated.is_some()
                && resp.is_truncated.unwrap()
                && resp.next_continuation_token.is_some()
            {
                Ok(ListRet::ToBeContinue(
                    resp.next_continuation_token.unwrap(),
                    list_items,
                ))
            } else if !list_items.is_empty() {
                Ok(ListRet::Finished(Some(list_items)))
            } else {
                Ok(ListRet::Finished(None))
            }
        } else {
            Ok(ListRet::Finished(None))
        }
    }
}
// n8fa3a9fcdd97d0240d81f88fc0bea13e,nb34cce81dfa08507714648572703b5da,nbbe3552bf67515e27375b33493b6ae3e,n74adfc89edfba49f3261a87e1e4c4eb2,n6ced609e52160cd31db8a0b74b074d43,na3f69fb9b39a22b53f44117a03994423,nac1dbb3fa5784b8b36003a6adf80e118,n3f8a604b68115e48c34daafb1b76a457,n352dedbf9906dfdb90ec9871255cde68,n71f709493d01f3c91f4f76e9da7934d0,n323d6c2ee52f159ab00aec37e6230550,n5104d65e21a7b23bd3c69b4a51302fd4,n3f5388314c6835854659b3aa52127d4a,n69199fad22a4e99156e9d00aa35139c2,nb9536b057cd529bb8811797ef49b80b8,nd3e1c6003da0d3175f7e4bc0a10f220a,n687d9d0bc90bff58dd599bf3b7342dcc,n53cf88190e239e5511ea9d87c1fde5cf,n9819ea31036b8514de68830515de004f,n816c14a1616792b215f034dcc89d15ed,ncd483580ce0c288e453499758c733c35,nb260a4b357b9c030114f9bd568932280,neb5984d16b88b5058b3cb6576272bcda,n5dc3b55e8b48bd3f8cc283faf8b75da6,nf585ddfdcac31dff3a980478f95b1ced,na6148c2ce16066fd4539df5a15c62191,n9e00069dd4f501d72b0aea5950e4f9e9,n62a51575b0ddb782d9cbc5c253fde274,n9a389484f8b02be1e1a5a7ffdf5072cb,nceaf788a68df17e4fa0117d940aa4fc7,n5dad5afede66552d8f6ca894f092cd29,
// n00074b0bbab3e40b48c2d92ac184ebc9
// ,nc88d9a198b215dd23df8177bee77de91,n336a6fe6cf1db29be53d6dcb72050407,n92264a59b1bd7608bb491e37a501f8a1,n9db26eaee5b1f5d088c418539fd654f3,n5b17e6bcda82ced8226931bf6d5df398,n5f7377a023b5429aadf87b9d923dbb3b,n20e3c650d3861d645050122e92b3a445,n303652d9c76239a78a67bb96f839daa9,nfb7d61bca1c44ca380e48bb0f37e79c1,n5c978a11cabab923eca72a953319c7e4,nb51082eb8c071c09e07bac744cf8827b,n185033cc695088c11ed812bf277b2c97,nfcce02eb6f1ad1f7e71d74fad3af9b1d,nd35181f77256dccdb5dfd8d5822989ae,n2b1e11899f10f59e77cae60c7e5dea01,ndd9d74efd2c9a905c083cc6de3eb92f9,ne7f5cfd820126af83c467cbde9fd33ca,nd50248256b752a9066dd36768c45e1c0,
// n0001555e7fc762b3fb0e0650f6212d0c
// ,n3e23b82f51597fcd8f443e8b80ecc7f8

pub enum ListRet {
    Finished(Option<Vec<ListItem>>),
    ToBeContinue(String, Vec<ListItem>),
}

#[derive(Debug)]
pub struct ListItem {
    pub key: Option<String>,
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,
    pub size: Option<i64>,
    pub storage_class: Option<String>,
}

#[derive(Default, Clone)]
pub struct ListArg {
    continuation_token: Option<String>,
    prefix: Option<String>,
    max_keys: Option<i64>,
}

impl ListArg {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn prefix<S: AsRef<str>>(self, s: S) -> Self {
        Self {
            prefix: Some(s.as_ref().to_owned()),
            continuation_token: None,
            max_keys: None,
        }
    }
    pub fn continuation_token<S: AsRef<str>>(self, s: S) -> Self {
        Self {
            prefix: self.prefix,
            continuation_token: Some(s.as_ref().to_owned()),
            max_keys: self.max_keys,
        }
    }
    pub fn limit(self, limit: ListLimit) -> Self {
        let ListLimit(u) = limit;
        Self {
            prefix: self.prefix,
            continuation_token: self.continuation_token,
            max_keys: Some(u as i64),
        }
    }
}

#[derive(Clone, Copy)]
pub struct ListLimit(usize);

use std::convert::TryFrom;
macro_rules! list_limit_try_from {
    (u => $type:ty) => {
        impl TryFrom<$type> for ListLimit {
            type Error = anyhow::Error;
            fn try_from(value: $type) -> Result<Self, Self::Error> {
                if value > 1000 {
                    Err(anyhow!("value must not be greater than 1000: {}", value))
                } else {
                    Ok(Self(value as usize))
                }
            }
        }
    };
}

list_limit_try_from!(u => u16);
list_limit_try_from!(u => usize);
list_limit_try_from!(u => u32);
list_limit_try_from!(u => u64);

pub struct ObjectToUploadBuilder {
    body: ByteStream,
    content_type: Option<String>,
    content_length: Option<i64>,
}

impl ObjectToUploadBuilder {
    pub fn from_file(src: tokio::fs::File) -> Self {
        let stream = into_bytes_stream(src);
        Self::from_bytestream(ByteStream::new(stream))
    }
    pub fn from_vecu8(body: Vec<u8>) -> Self {
        let body = Bytes::from(body);
        Self::from_bytes(body)
    }
    pub fn from_bytes(body: Bytes) -> Self {
        let stream = stream::once(async { Ok(body) });
        let body = ByteStream::new(stream);
        Self::from_bytestream(body)
    }
    pub fn from_bytestream(body: ByteStream) -> Self {
        Self {
            body,
            content_length: None,
            content_type: None,
        }
    }
    pub fn content_length(self, content_length: Option<i64>) -> Self {
        Self {
            body: self.body,
            content_type: self.content_type,
            content_length,
        }
    }
    pub fn content_type(self, content_type: Option<String>) -> Self {
        Self {
            body: self.body,
            content_type,
            content_length: self.content_length,
        }
    }
    pub fn build(self) -> ObjectToUpload {
        ObjectToUpload {
            body: self.body,
            content_type: self.content_type,
            content_length: self.content_length,
        }
    }
}
pub struct ObjectToUpload {
    pub body: ByteStream,
    pub content_type: Option<String>,
    pub content_length: Option<i64>,
}

// https://stackoverflow.com/questions/59318460/what-is-the-best-way-to-convert-an-asyncread-to-a-trystream-of-bytes/59327560#59327560

pub fn into_byte_stream<R>(r: R) -> impl Stream<Item = tokio::io::Result<u8>>
where
    R: AsyncRead,
{
    codec::FramedRead::new(r, codec::BytesCodec::new())
        .map_ok(|bytes| stream::iter(bytes).map(Ok))
        .try_flatten()
}

pub fn into_bytes_stream<R>(r: R) -> impl Stream<Item = tokio::io::Result<Bytes>>
where
    R: AsyncRead,
{
    codec::FramedRead::new(r, codec::BytesCodec::new()).map_ok(|bytes| bytes.freeze())
}

// ==================================================================================
// ==================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use std::path::Path;
    use tokio::io::AsyncReadExt;

    use std::convert::TryInto;
    #[tokio::test]
    async fn test_list() -> Result<()> {
        dotenv()?;
        let _ = pretty_env_logger::try_init_timed();
        let_from_env!(bucket, S3_BUCKET);
        let s3b = S3BucketBuilder::new(bucket).build()?;
        let mut list_items = vec![];
        let_from_env!(prefix, LIST_PREFIX);
        let_from_env!(limit, LIST_LIMIT, u16);
        let limit = limit.try_into()?;
        let mut list_arg = ListArg::new().prefix(&prefix).limit(limit);
        loop {
            let ret = s3b.list(list_arg).await?;
            match ret {
                ListRet::ToBeContinue(continuation_token, mut items) => {
                    list_items.append(&mut items);
                    list_arg = ListArg::new()
                        .prefix(&prefix)
                        .limit(limit)
                        .continuation_token(continuation_token);
                }
                ListRet::Finished(items_opt) => {
                    if let Some(mut items) = items_opt {
                        list_items.append(&mut items);
                    }
                    break;
                }
            }
        }
        info!("list_items({}): {:#?}", list_items.len(), list_items);
        Ok(())
    }

    #[tokio::test]
    async fn test_upload_file() -> Result<()> {
        dotenv().context("file .env")?;
        let _ = pretty_env_logger::try_init_timed();
        let_from_env!(bucket, S3_BUCKET);
        let s3b = S3BucketBuilder::new(bucket).build()?;

        let_from_env!(test_files, TEST_FILES);
        for test_filepath in test_files.split(",") {
            let filepath = Path::new(&test_filepath);
            let key = filepath.file_name().unwrap().to_string_lossy().to_string();
            while s3b.head(key.clone()).await?.is_some() {
                s3b.delete(key.clone()).await?;
                tokio::time::delay_for(tokio::time::Duration::from_millis(500)).await;
            }
            let mut file = tokio::fs::File::open(filepath).await?;
            let mime_sniff::Ret {
                content_type,
                content_length,
            } = mime_sniff::tokio_file(&mut file).await?;

            info!(
                "filepath: {:?}, content_type: {:?}, content_length: {:?}",
                filepath, content_type, content_length
            );
            let object_to_upload = ObjectToUploadBuilder::from_file(file)
                .content_length(content_length)
                .content_type(content_type.clone())
                .build();
            s3b.upload(key.clone(), object_to_upload).await?;
            let head = s3b
                .head(key.clone())
                .await?
                .expect(&format!("key {} must exist", key));
            assert_eq!(head.content_type, content_type);
            assert_eq!(head.content_length, content_length);
        }
        // s3b.list(ListArg::new());
        Ok(())
    }

    #[tokio::test]
    async fn test_upload_vecu8() -> Result<()> {
        dotenv()?;
        let _ = pretty_env_logger::try_init_timed();
        let_from_env!(bucket, S3_BUCKET);
        let s3b = S3BucketBuilder::new(bucket).build()?;

        let_from_env!(test_files, TEST_FILES);
        for test_filepath in test_files.split(",") {
            let filepath = Path::new(&test_filepath);
            let key = filepath.file_name().unwrap().to_string_lossy().to_string();
            // while s3b.exists(key.clone()).await? {
            while s3b.head(key.clone()).await?.is_some() {
                s3b.delete(key.clone()).await?;
                tokio::time::delay_for(tokio::time::Duration::from_millis(500)).await;
            }
            let mut file = tokio::fs::File::open(filepath).await?;

            let mut buffer = vec![];
            file.read_to_end(&mut buffer).await?;

            let mime_sniff::Ret {
                content_type,
                content_length,
            } = mime_sniff::slice(&buffer);

            info!(
                "filepath: {:?}, content_type: {:?}, content_length: {:?}",
                filepath, content_type, content_length
            );
            let object_to_upload = ObjectToUploadBuilder::from_vecu8(buffer)
                .content_length(content_length)
                .content_type(content_type.clone())
                .build();
            s3b.upload(key.clone(), object_to_upload).await?;
            let head = s3b
                .head(key.clone())
                .await?
                .expect(&format!("key {} must exist", key));
            assert_eq!(head.content_type, content_type);
            assert_eq!(head.content_length, content_length);
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_list_photo() -> Result<()> {
        dotenv()?;
        let _ = pretty_env_logger::try_init_timed();
        info!("test_list_photo");
        let_from_env!(bucket, S3_BUCKET);
        let s3b = S3BucketBuilder::new(bucket).build()?;
        let mut list_items = vec![];
        let_from_env!(prefix, LIST_PREFIX);
        let_from_env!(limit, LIST_LIMIT, u16);
        let_from_env!(max_count, LIST_MAX_COUNT, usize);
        let limit = limit.try_into()?;
        let mut list_arg = ListArg::new().prefix(&prefix).limit(limit);
        let start = std::time::Instant::now();
        loop {
            info!("will list");
            let ret = s3b.list(list_arg).await?;
            match ret {
                ListRet::ToBeContinue(continuation_token, mut items) => {
                    info!("got {} items", items.len());
                    list_items.append(&mut items);
                    list_arg = ListArg::new()
                        .prefix(&prefix)
                        .limit(limit)
                        .continuation_token(continuation_token);
                }
                ListRet::Finished(items_opt) => {
                    if let Some(mut items) = items_opt {
                        info!("got {} items", items.len());
                        list_items.append(&mut items);
                    }
                    break;
                }
            }
            if list_items.len() >= max_count {
                break;
            }
        }
        let show_max_count = 5;
        info!(
            "{}, list_items({}): {:#?}",
            arrange_millis::get(std::time::Instant::now().duration_since(start).as_millis()),
            list_items.len(),
            if list_items.len() <= show_max_count {
                list_items
            } else {
                list_items.into_iter().take(show_max_count).collect()
            }
        );
        Ok(())
    }
}

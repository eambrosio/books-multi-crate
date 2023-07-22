use apache_avro::Schema;
use opentelemetry::propagation::{Extractor, Injector};
use rdkafka::message::{BorrowedHeaders, Headers, OwnedHeaders};
use schema_registry_converter::{
    async_impl::schema_registry::{post_schema, SrSettings},
    avro_common::get_supplied_schema,
    error::SRCError,
    schema_registry_common::RegisteredSchema,
};
use std::str;

pub struct HeaderInjector<'a>(pub &'a mut OwnedHeaders);

impl<'a> Injector for HeaderInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        let mut new_headers = OwnedHeaders::new().insert(rdkafka::message::Header {
            key,
            value: Some(&value),
        });

        for header in self.0.iter() {
            let value = String::from_utf8(header.value.unwrap().to_vec()).unwrap();
            new_headers = new_headers.insert(rdkafka::message::Header {
                key: header.key,
                value: Some(&value),
            });
        }

        self.0.clone_from(&new_headers)
    }
}

pub struct HeaderExtractor<'a>(pub &'a BorrowedHeaders);

impl<'a> Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        if let Some(header) = self.0.iter().find(|header| header.key == key) {
            return Some(str::from_utf8(header.value.unwrap()).unwrap());
        }
        None
        // for idx in 0..self.0.count() {
        //     if let Ok(val) = self.0.get_as::<str>(idx) {
        //         if val.key == key {
        //             return val.value;
        //         }
        //     }
        // }
        // None
    }

    fn keys(&self) -> Vec<&str> {
        self.0.iter().map(|kv| kv.key).collect::<Vec<_>>()
    }
}

pub async fn register_schema(
    schema_registry_url: String,
    subject: String,
    schema: Schema,
) -> Result<RegisteredSchema, SRCError> {
    let sr_settings = SrSettings::new(schema_registry_url);
    let supplied_schema = *get_supplied_schema(&schema);
    post_schema(&sr_settings, subject, supplied_schema).await
}

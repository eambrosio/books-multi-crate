use crate::util::{get_span, initialize_headers, HeaderInjector};
use apache_avro::AvroSchema;

use opentelemetry::trace::TraceContextExt;
use opentelemetry::{global, Context};

use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use schema_registry_converter::async_impl::easy_avro::EasyAvroEncoder;
use schema_registry_converter::async_impl::schema_registry::SrSettings;
use schema_registry_converter::avro_common::get_supplied_schema;
use schema_registry_converter::schema_registry_common::SubjectNameStrategy;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

#[derive(Clone)]
pub struct KafkaProducer {
    producer: FutureProducer,
    avro_encoder: Arc<EasyAvroEncoder>,
    topic: String,
}

impl KafkaProducer {
    pub fn new(bootstrap_servers: String, schema_registry_url: String, topic: String) -> Self {
        let producer = ClientConfig::new()
            .set("bootstrap.servers", bootstrap_servers)
            .set("produce.offset.report", "true")
            .set("message.timeout.ms", "5000")
            .set("queue.buffering.max.messages", "10")
            .create()
            .expect("Producer creation error");
        let sr_settings = SrSettings::new(schema_registry_url);
        let avro_encoder = EasyAvroEncoder::new(sr_settings);

        Self {
            producer,
            avro_encoder: Arc::new(avro_encoder),
            topic,
        }
    }

    pub async fn produce<T: Serialize + AvroSchema>(&self, key: String, payload: T) -> bool {
        let value_strategy = SubjectNameStrategy::TopicNameStrategyWithSchema(
            self.topic.clone(),
            true,
            get_supplied_schema(&T::get_schema()),
        );
        let payload = match self
            .avro_encoder
            .clone()
            .encode_struct(payload, &value_strategy)
            .await
        {
            Ok(v) => v,
            Err(e) => panic!("Error getting payload: {}", e),
        };

        let span = get_span(&payload, self.topic.clone(), "producer", "produce_to_kafka");
        let context = Context::current_with_span(span);
        let mut headers = initialize_headers();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&context, &mut HeaderInjector(&mut headers))
        });

        let record = FutureRecord::to(&self.topic)
            .payload(&payload)
            .key(&key)
            .headers(headers);
        let delivery_status = self.producer.send(record, Duration::from_secs(5)).await;
        delivery_status.map_or_else(
            |e| {
                error!("{}", e.0.to_string());
                return false;
            },
            |_| {
                info!("message delivered");
                return true;
            },
        )
    }
}

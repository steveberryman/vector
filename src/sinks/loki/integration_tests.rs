use super::config::{LokiConfig, OutOfOrderAction};
use crate::{
    config::{log_schema, SinkConfig},
    sinks::util::test::load_sink,
    sinks::VectorSink,
    template::Template,
    test_util::{
        random_events_with_stream, random_lines, random_lines_with_stream,
        random_updated_events_with_stream,
    },
};
use bytes::Bytes;
use chrono::{DateTime, Duration, Utc};
use futures::stream;
use shared::encode_logfmt;
use std::convert::TryFrom;
use vector_core::event::{BatchNotifier, BatchStatus, Event};

async fn build_sink(encoding: &str) -> (uuid::Uuid, VectorSink) {
    let stream = uuid::Uuid::new_v4();

    let config = format!(
        r#"
            endpoint = "http://localhost:3100"
            labels = {{test_name = "placeholder"}}
            encoding = "{}"
            remove_timestamp = false
            tenant_id = "default"
        "#,
        encoding
    );

    let (mut config, cx) = load_sink::<LokiConfig>(&config).unwrap();

    let test_name = config
        .labels
        .get_mut(&Template::try_from("test_name").unwrap())
        .unwrap();
    assert_eq!(test_name.get_ref(), &Bytes::from("placeholder"));

    *test_name = Template::try_from(stream.to_string()).unwrap();

    let (sink, _) = config.build(cx).await.unwrap();

    (stream, sink)
}

#[tokio::test]
async fn text() {
    let (stream, sink) = build_sink("text").await;

    let (batch, mut receiver) = BatchNotifier::new_with_receiver();
    let (lines, events) = random_lines_with_stream(100, 10, Some(batch));
    let _ = sink.run(events).await.unwrap();
    assert_eq!(receiver.try_recv(), Ok(BatchStatus::Delivered));

    tokio::time::sleep(tokio::time::Duration::new(1, 0)).await;

    let (_, outputs) = fetch_stream(stream.to_string(), "default").await;
    assert_eq!(lines.len(), outputs.len());
    for (i, output) in outputs.iter().enumerate() {
        assert_eq!(output, &lines[i]);
    }
}

#[tokio::test]
async fn json() {
    let (stream, sink) = build_sink("json").await;

    let (batch, mut receiver) = BatchNotifier::new_with_receiver();
    let (lines, events) = random_events_with_stream(100, 10, Some(batch));
    let _ = sink.run(events).await.unwrap();
    assert_eq!(receiver.try_recv(), Ok(BatchStatus::Delivered));

    tokio::time::sleep(tokio::time::Duration::new(1, 0)).await;

    let (_, outputs) = fetch_stream(stream.to_string(), "default").await;
    assert_eq!(lines.len(), outputs.len());
    for (i, output) in outputs.iter().enumerate() {
        let expected_json = serde_json::to_string(&lines[i].as_log()).unwrap();
        assert_eq!(output, &expected_json);
    }
}

// https://github.com/timberio/vector/issues/7815
#[tokio::test]
async fn json_nested_fields() {
    let (stream, sink) = build_sink("json").await;

    let (batch, mut receiver) = BatchNotifier::new_with_receiver();
    let (lines, events) =
        random_updated_events_with_stream(100, 10, Some(batch), |(_index, mut event)| {
            let log = event.as_mut_log();
            log.insert("foo.bar", "baz");
            event
        });
    let _ = sink.run(events).await.unwrap();
    assert_eq!(receiver.try_recv(), Ok(BatchStatus::Delivered));

    tokio::time::sleep(tokio::time::Duration::new(1, 0)).await;

    let (_, outputs) = fetch_stream(stream.to_string(), "default").await;
    assert_eq!(lines.len(), outputs.len());
    for (i, output) in outputs.iter().enumerate() {
        let expected_json = serde_json::to_string(&lines[i].as_log()).unwrap();
        assert_eq!(output, &expected_json);
    }
}

#[tokio::test]
async fn logfmt() {
    let (stream, sink) = build_sink("logfmt").await;

    let (batch, mut receiver) = BatchNotifier::new_with_receiver();
    let (lines, events) = random_events_with_stream(100, 10, Some(batch));
    let _ = sink.run(events).await.unwrap();
    assert_eq!(receiver.try_recv(), Ok(BatchStatus::Delivered));

    tokio::time::sleep(tokio::time::Duration::new(1, 0)).await;

    let (_, outputs) = fetch_stream(stream.to_string(), "default").await;
    assert_eq!(lines.len(), outputs.len());
    for (i, output) in outputs.iter().enumerate() {
        let expected_logfmt =
            encode_logfmt::to_string(lines[i].clone().into_log().into_parts().0).unwrap();
        assert_eq!(output, &expected_logfmt);
    }
}

#[tokio::test]
async fn many_streams() {
    let stream1 = uuid::Uuid::new_v4();
    let stream2 = uuid::Uuid::new_v4();

    let (config, cx) = load_sink::<LokiConfig>(
        r#"
            endpoint = "http://localhost:3100"
            labels = {test_name = "{{ stream_id }}"}
            encoding = "text"
            tenant_id = "default"
        "#,
    )
    .unwrap();

    let (sink, _) = config.build(cx).await.unwrap();

    let (batch, mut receiver) = BatchNotifier::new_with_receiver();
    let (lines, events) =
        random_updated_events_with_stream(100, 10, Some(batch), |(i, mut event)| {
            if i < 10 {
                let log = event.as_mut_log();
                if i % 2 == 0 {
                    log.insert("stream_id", stream1.to_string());
                } else {
                    log.insert("stream_id", stream2.to_string());
                }
            }
            event
        });

    let _ = sink.run(events).await.unwrap();
    assert_eq!(receiver.try_recv(), Ok(BatchStatus::Delivered));

    tokio::time::sleep(tokio::time::Duration::new(1, 0)).await;

    let (_, outputs1) = fetch_stream(stream1.to_string(), "default").await;
    let (_, outputs2) = fetch_stream(stream2.to_string(), "default").await;

    assert_eq!(outputs1.len() + outputs2.len(), lines.len());

    for (i, output) in outputs1.iter().enumerate() {
        let index = (i % 5) * 2;
        let message = lines[index]
            .as_log()
            .get(log_schema().message_key())
            .unwrap()
            .to_string_lossy();
        assert_eq!(output, &message);
    }

    for (i, output) in outputs2.iter().enumerate() {
        let index = ((i % 5) * 2) + 1;
        let message = lines[index]
            .as_log()
            .get(log_schema().message_key())
            .unwrap()
            .to_string_lossy();
        assert_eq!(output, &message);
    }
}

#[tokio::test]
async fn interpolate_stream_key() {
    let stream = uuid::Uuid::new_v4();

    let (mut config, cx) = load_sink::<LokiConfig>(
        r#"
            endpoint = "http://localhost:3100"
            labels = {"{{ stream_key }}" = "placeholder"}
            encoding = "text"
            tenant_id = "default"
        "#,
    )
    .unwrap();
    config.labels.insert(
        Template::try_from("{{ stream_key }}").unwrap(),
        Template::try_from(stream.to_string()).unwrap(),
    );

    let (sink, _) = config.build(cx).await.unwrap();

    let (batch, mut receiver) = BatchNotifier::new_with_receiver();
    let (lines, events) =
        random_updated_events_with_stream(100, 10, Some(batch), |(i, mut event)| {
            if i < 10 {
                event.as_mut_log().insert("stream_key", "test_name");
            }
            event
        });

    let _ = sink.run(events).await.unwrap();
    assert_eq!(receiver.try_recv(), Ok(BatchStatus::Delivered));

    tokio::time::sleep(tokio::time::Duration::new(1, 0)).await;

    let (_, outputs) = fetch_stream(stream.to_string(), "default").await;

    assert_eq!(outputs.len(), lines.len());

    for (i, output) in outputs.iter().enumerate() {
        let message = lines[i]
            .as_log()
            .get(log_schema().message_key())
            .unwrap()
            .to_string_lossy();
        assert_eq!(output, &message);
    }
}

#[tokio::test]
async fn many_tenants() {
    let stream = uuid::Uuid::new_v4();

    let (mut config, cx) = load_sink::<LokiConfig>(
        r#"
            endpoint = "http://localhost:3100"
            labels = {test_name = "placeholder"}
            encoding = "text"
            tenant_id = "{{ tenant_id }}"
        "#,
    )
    .unwrap();

    let test_name = config
        .labels
        .get_mut(&Template::try_from("test_name").unwrap())
        .unwrap();
    assert_eq!(test_name.get_ref(), &Bytes::from("placeholder"));

    *test_name = Template::try_from(stream.to_string()).unwrap();

    let (sink, _) = config.build(cx).await.unwrap();

    let lines = random_lines(100).take(10).collect::<Vec<_>>();

    let mut events = lines
        .clone()
        .into_iter()
        .map(Event::from)
        .collect::<Vec<_>>();

    for i in 0..10 {
        let event = events.get_mut(i).unwrap();

        if i % 2 == 0 {
            event.as_mut_log().insert("tenant_id", "tenant1");
        } else {
            event.as_mut_log().insert("tenant_id", "tenant2");
        }
    }

    let _ = sink.run(&mut stream::iter(events)).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::new(1, 0)).await;

    let (_, outputs1) = fetch_stream(stream.to_string(), "tenant1").await;
    let (_, outputs2) = fetch_stream(stream.to_string(), "tenant2").await;

    assert_eq!(outputs1.len() + outputs2.len(), lines.len());

    for (i, output) in outputs1.iter().enumerate() {
        let index = (i % 5) * 2;
        assert_eq!(output, &lines[index]);
    }

    for (i, output) in outputs2.iter().enumerate() {
        let index = ((i % 5) * 2) + 1;
        assert_eq!(output, &lines[index]);
    }
}

#[tokio::test]
async fn out_of_order_drop() {
    let batch_size = 5;
    let lines = random_lines(100).take(10).collect::<Vec<_>>();
    let mut events = lines
        .clone()
        .into_iter()
        .map(Event::from)
        .collect::<Vec<_>>();

    let base = chrono::Utc::now() - Duration::seconds(20);
    for (i, event) in events.iter_mut().enumerate() {
        let log = event.as_mut_log();
        log.insert(
            log_schema().timestamp_key(),
            base + Duration::seconds(i as i64),
        );
    }
    // first event of the second batch is out-of-order.
    events[batch_size]
        .as_mut_log()
        .insert(log_schema().timestamp_key(), base);

    let mut expected = events.clone();
    expected.remove(batch_size);

    test_out_of_order_events(OutOfOrderAction::Drop, batch_size, events, expected).await;
}

#[tokio::test]
async fn out_of_order_rewrite() {
    let batch_size = 5;
    let lines = random_lines(100).take(10).collect::<Vec<_>>();
    let mut events = lines
        .clone()
        .into_iter()
        .map(Event::from)
        .collect::<Vec<_>>();

    let base = chrono::Utc::now() - Duration::seconds(20);
    for (i, event) in events.iter_mut().enumerate() {
        let log = event.as_mut_log();
        log.insert(
            log_schema().timestamp_key(),
            base + Duration::seconds(i as i64),
        );
    }
    // first event of the second batch is out-of-order.
    events[batch_size]
        .as_mut_log()
        .insert(log_schema().timestamp_key(), base);

    let mut expected = events.clone();
    let time = get_timestamp(&expected[batch_size - 1]);
    // timestamp is rewriten with latest timestamp of the first batch
    expected[batch_size]
        .as_mut_log()
        .insert(log_schema().timestamp_key(), time);

    test_out_of_order_events(
        OutOfOrderAction::RewriteTimestamp,
        batch_size,
        events,
        expected,
    )
    .await;
}

#[tokio::test]
async fn out_of_order_per_partition() {
    let batch_size = 2;
    let big_lines = random_lines(1_000_000).take(2);
    let small_lines = random_lines(1).take(20);
    let mut events = big_lines
        .into_iter()
        .chain(small_lines)
        .map(Event::from)
        .collect::<Vec<_>>();

    let base = chrono::Utc::now() - Duration::seconds(30);
    for (i, event) in events.iter_mut().enumerate() {
        let log = event.as_mut_log();
        log.insert(
            log_schema().timestamp_key(),
            base + Duration::seconds(i as i64),
        );
    }

    // So, since all of the events are of the same partition, and if there is concurrency,
    // then if ordering inside paritions isn't upheld, the big line events will take longer
    // time to flush than small line events so loki will receive smaller ones before large
    // ones hence out of order events.
    test_out_of_order_events(OutOfOrderAction::Drop, batch_size, events.clone(), events).await;
}

async fn test_out_of_order_events(
    action: OutOfOrderAction,
    batch_size: usize,
    events: Vec<Event>,
    expected: Vec<Event>,
) {
    crate::test_util::trace_init();
    let stream = uuid::Uuid::new_v4();

    let (mut config, cx) = load_sink::<LokiConfig>(
        r#"
            endpoint = "http://localhost:3100"
            labels = {test_name = "placeholder"}
            encoding = "text"
            tenant_id = "default"
        "#,
    )
    .unwrap();
    config.out_of_order_action = action;
    config.labels.insert(
        Template::try_from("test_name").unwrap(),
        Template::try_from(stream.to_string()).unwrap(),
    );
    config.batch.max_events = Some(batch_size);
    config.batch.max_bytes = Some(4_000_000);

    let (sink, _) = config.build(cx).await.unwrap();
    sink.run(&mut stream::iter(events.clone())).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::new(1, 0)).await;

    let (timestamps, outputs) = fetch_stream(stream.to_string(), "default").await;
    assert_eq!(expected.len(), outputs.len());
    assert_eq!(expected.len(), timestamps.len());
    for (i, output) in outputs.iter().enumerate() {
        assert_eq!(
            &expected[i]
                .as_log()
                .get(log_schema().message_key())
                .unwrap()
                .to_string_lossy(),
            output,
        )
    }
    for (i, ts) in timestamps.iter().enumerate() {
        assert_eq!(get_timestamp(&expected[i]).timestamp_nanos(), *ts);
    }
}

fn get_timestamp(event: &Event) -> DateTime<Utc> {
    *event
        .as_log()
        .get(log_schema().timestamp_key())
        .unwrap()
        .as_timestamp()
        .unwrap()
}

async fn fetch_stream(stream: String, tenant: &str) -> (Vec<i64>, Vec<String>) {
    let query = format!("%7Btest_name%3D\"{}\"%7D", stream);
    let query = format!(
        "http://localhost:3100/loki/api/v1/query_range?query={}&direction=forward",
        query
    );

    let res = reqwest::Client::new()
        .get(&query)
        .header("X-Scope-OrgID", tenant)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    // The response type follows this api https://github.com/grafana/loki/blob/master/docs/api.md#get-lokiapiv1query_range
    // where the result type is `streams`.
    let data = res.json::<serde_json::Value>().await.unwrap();

    // TODO: clean this up or explain it via docs
    let results = data
        .get("data")
        .unwrap()
        .get("result")
        .unwrap()
        .as_array()
        .unwrap();

    let values = results[0].get("values").unwrap().as_array().unwrap();

    // the array looks like: [ts, line].
    let timestamps = values
        .iter()
        .map(|v| v[0].as_str().unwrap().parse().unwrap())
        .collect();
    let lines = values
        .iter()
        .map(|v| v[1].as_str().unwrap().to_string())
        .collect();
    (timestamps, lines)
}

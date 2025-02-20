package metadata

components: sinks: influxdb_logs: {
	title: "InfluxDB Logs"

	classes: {
		commonly_used: false
		delivery:      "at_least_once"
		development:   "stable"
		egress_method: "batch"
		service_providers: ["InfluxData"]
		stateful: false
	}

	features: {
		buffer: enabled:      true
		healthcheck: enabled: true
		send: {
			batch: {
				enabled:      true
				common:       false
				max_bytes:    1049000
				timeout_secs: 1
			}
			compression: enabled: false
			encoding: {
				enabled: true
				codec: enabled: false
			}
			request: {
				enabled: true
				headers: false
			}
			tls: sinks._influxdb.features.send.tls
			to:  sinks._influxdb.features.send.to
		}
	}

	support: {
		requirements: []
		warnings: []
		notices: []
	}

	configuration: sinks._influxdb.configuration & {
		namespace: {
			description: "A prefix that will be added to all logs names."
			groups: ["v1", "v2"]
			required: true
			type: string: {
				examples: ["service"]
			}
		}
	}

	input: {
		logs:    true
		metrics: null
	}

	how_it_works: {
		mapping: {
			title: "Mapping Log Fields"
			body:  """
				InfluxDB uses [line protocol](\(urls.influxdb_line_protocol)) to write data points. It is a text-based format that provides the measurement, tag set, field set, and timestamp of a data point.

				A `Log Event` event contains an arbitrary set of fields (key/value pairs) that describe the event.

				The following matrix outlines how Log Event fields are mapped into InfluxDB Line Protocol:

				| Field         | Line Protocol     |                                                                                                                                                 |
				|---------------|-------------------|
				| host          | tag               |
				| message       | field             |
				| source_type   | tag               |
				| timestamp     | timestamp         |
				| [custom-key]  | field             |

				The default behavior can be overridden by a `tags` configuration.
				"""

			sub_sections: [
				{
					title: "Mapping Example"
					body: """
						The following event:

						```js
						{
						  "host": "my.host.com",
						  "message": "<13>Feb 13 20:07:26 74794bfb6795 root[8539]: i am foobar",
						  "timestamp": "2019-11-01T21:15:47+00:00",
						  "custom_field": "custom_value"
						}
						```

						Will be mapped to Influx's line protocol:

						```influxdb_line_protocol
						ns.vector,host=my.host.com,metric_type=logs custom_field="custom_value",message="<13>Feb 13 20:07:26 74794bfb6795 root[8539]: i am foobar" 1572642947000000000
						```
						"""
				},
			]
		}
	}

	telemetry: metrics: {
		component_sent_bytes_total:       components.sources.internal_metrics.output.metrics.component_sent_bytes_total
		component_sent_events_total:      components.sources.internal_metrics.output.metrics.component_sent_events_total
		component_sent_event_bytes_total: components.sources.internal_metrics.output.metrics.component_sent_event_bytes_total
		events_out_total:                 components.sources.internal_metrics.output.metrics.events_out_total
	}
}

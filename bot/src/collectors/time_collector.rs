use crate::types::{Collector, CollectorStream};
use anyhow::Result;
use async_trait::async_trait;
use tokio::time::{self, Instant};
use std::{time::{Duration, SystemTime, UNIX_EPOCH}};
use tokio_stream::{wrappers::IntervalStream, StreamExt};

/// A collector that listens for new blockchain event logs based on a [Filter](Filter),
/// and generates a stream of [events](Log).
pub struct TimerCollector {
    period_miliseconds: u64
}

impl TimerCollector {
    pub fn new(period_miliseconds: u64 ) -> Self {
        Self { period_miliseconds }
    }
}

#[async_trait]
impl Collector<u64> for TimerCollector
where
{
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, u64>> {
        let base_instant = Instant::now();
        let base_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        let stream = IntervalStream::new(time::interval(Duration::from_millis(self.period_miliseconds)));
        let stream = stream.map(move |x| (x.duration_since(base_instant) + base_time).as_secs());
        Ok(Box::pin(stream))
    }
}

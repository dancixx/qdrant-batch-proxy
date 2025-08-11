use std::sync::Arc;

use anyhow::Result;
use fastembed::TextEmbedding;
use tokio::{
    sync::Mutex,
    time::{Instant, sleep_until},
};

pub struct BatchEngine {
    // Maximum time to wait for a batch to be full before sending it in milliseconds
    pub max_wait_time: usize,
    // Maximum size of a batch before sending it
    pub max_batch_size: usize,
}

impl BatchEngine {
    #[must_use]
    pub fn new() -> Result<Self> {
        Ok(BatchEngine {
            max_wait_time: std::env::var("MAX_WAIT_TIME")?.parse::<usize>()?,
            max_batch_size: std::env::var("MAX_BATCH_SIZE")?.parse::<usize>()?,
        })
    }

    pub async fn run(
        &self,
        mut rx: tokio::sync::mpsc::Receiver<BatchItem>,
        embedder: Arc<Mutex<TextEmbedding>>,
    ) {
        loop {
            let Some(first) = rx.recv().await else {
                break;
            };

            let deadline =
                Instant::now() + std::time::Duration::from_millis(self.max_wait_time as u64);
            let mut batch = Vec::<BatchItem>::with_capacity(self.max_batch_size);
            batch.push(first);

            let sleep = sleep_until(deadline);
            tokio::pin!(sleep);

            // wait for the next item or timeout
            while batch.len() < self.max_batch_size {
                let now = Instant::now();
                if now >= deadline {
                    break;
                }
                let next = tokio::select! {
                    _ = &mut sleep => None,
                    v = rx.recv() => v
                };
                match next {
                    Some(item) => batch.push(item),
                    None => break,
                }
            }

            let counts = batch.iter().map(|i| i.input.len()).collect::<Vec<_>>();
            let total = counts.iter().sum::<usize>();

            // flatten inputs for embedding
            let mut flat_inputs = Vec::with_capacity(total);
            for it in &batch {
                for input in &it.input {
                    flat_inputs.push(input.clone());
                }
            }

            let embs = embedder.lock().await.embed(flat_inputs, Some(total));
            match embs {
                Ok(embeddings) => {
                    if embeddings.len() != total {
                        for item in batch.into_iter() {
                            std::mem::drop(item.tx.send(Err(anyhow::anyhow!(
                                "mismatched upstream count: got {} expected {}",
                                embeddings.len(),
                                total
                            ))));
                        }
                        continue;
                    }

                    // send back embeddings through oneshot channel for certain requests
                    let mut offset = 0usize;
                    for (i, item) in batch.into_iter().enumerate() {
                        let n = counts[i];
                        let slice = &embeddings[offset..offset + n];
                        offset += n;

                        let out = slice.to_vec().into_iter().collect::<Vec<_>>();
                        let _ = item.tx.send(Ok(out));
                    }
                }
                Err(err) => {
                    for item in batch.into_iter() {
                        std::mem::drop(
                            item.tx
                                .send(Err(anyhow::anyhow!(format!("upstream error: {err:?}")))),
                        );
                    }
                }
            }
        }
    }
}

pub struct BatchItem {
    pub input: Vec<String>,
    pub tx: tokio::sync::oneshot::Sender<anyhow::Result<Vec<Vec<f32>>>>,
}

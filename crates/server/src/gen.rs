//! Worker-thread chunk generation. Jobs go in over a channel, finished chunks
//! come back over another; the server never blocks on either.

use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use terravibe_sim::{Chunk, ChunkPos, WorldSize, generate_chunk_sized};

/// Most chunks generating at once.
pub const MAX_IN_FLIGHT: usize = 64;

pub struct Generator {
    jobs: Sender<ChunkPos>,
    done: Receiver<(ChunkPos, Arc<Chunk>)>,
    in_flight: HashSet<ChunkPos>,
}

impl Generator {
    pub fn new(seed: u32, size: WorldSize) -> Self {
        let (jobs, job_rx) = mpsc::channel::<ChunkPos>();
        let (done_tx, done) = mpsc::channel();
        let job_rx = Arc::new(Mutex::new(job_rx));
        let n = std::thread::available_parallelism()
            .map_or(1, |n| n.get())
            .saturating_sub(1)
            .max(1);
        for _ in 0..n {
            let rx = Arc::clone(&job_rx);
            let tx = done_tx.clone();
            std::thread::spawn(move || {
                loop {
                    let job = rx.lock().unwrap().recv();
                    let Ok(pos) = job else { break };
                    let chunk = Arc::new(generate_chunk_sized(seed, pos, size));
                    if tx.send((pos, chunk)).is_err() {
                        break;
                    }
                }
            });
        }
        Self {
            jobs,
            done,
            in_flight: HashSet::new(),
        }
    }

    pub fn is_in_flight(&self, pos: ChunkPos) -> bool {
        self.in_flight.contains(&pos)
    }

    /// Queue a chunk; returns false if at capacity or already queued.
    pub fn request(&mut self, pos: ChunkPos) -> bool {
        if self.in_flight.len() >= MAX_IN_FLIGHT || !self.in_flight.insert(pos) {
            return false;
        }
        if self.jobs.send(pos).is_err() {
            self.in_flight.remove(&pos);
            return false;
        }
        true
    }

    /// Finished chunks, without blocking.
    pub fn drain(&mut self) -> Vec<(ChunkPos, Arc<Chunk>)> {
        let mut out = Vec::new();
        while let Ok((pos, chunk)) = self.done.try_recv() {
            self.in_flight.remove(&pos);
            out.push((pos, chunk));
        }
        out
    }
}

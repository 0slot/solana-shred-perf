use std::collections::HashMap;
use std::time::{Duration, Instant};
use clap::Parser;
use log::{info, error};
use solana_ledger::shred::{Shred, ShredId};
use tokio::net::UdpSocket;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long)]
    pub name_0: String,
    #[clap(long)]
    pub port_0: u16,
    #[clap(short, long)]
    pub name_1: String,
    #[clap(short, long)]
    pub port_1: u16,
    #[clap(long, default_value = "60")]
    pub timeout_secs: u64,
}

#[derive(Debug)]
enum ProcessorEvent {
    ShredReceived {
        port_id: u8,
        name: Arc<str>,
        shred_id: ShredId,
        timestamp: Instant,
    },
    Cleanup,
    StatsTick,
}

struct ProcessorState {
    port0_data: HashMap<ShredId, Instant>,
    port1_data: HashMap<ShredId, Instant>,
    matched_pairs: usize,
    delays: Vec<Duration>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    let args = Args::parse();

    let (processor_tx, mut processor_rx) = mpsc::channel(4096);

    let port0_task = start_port_listener(0, args.name_0.clone().into(), args.port_0, processor_tx.clone());
    let port1_task = start_port_listener(1, args.name_1.clone().into(), args.port_1, processor_tx.clone());

    let timer_task = {
        let processor_tx = processor_tx.clone();
        tokio::spawn(async move {
            let mut cleanup_interval = time::interval(Duration::from_secs(args.timeout_secs));
            let mut stats_interval = time::interval(Duration::from_secs(10));

            loop {
                tokio::select! {
                    _ = cleanup_interval.tick() => {
                        processor_tx.send(ProcessorEvent::Cleanup).await.ok();
                    }
                    _ = stats_interval.tick() => {
                        processor_tx.send(ProcessorEvent::StatsTick).await.ok();
                    }
                }
            }
        })
    };

    let processor_task = tokio::spawn(async move {
        let mut state = ProcessorState {
            port0_data: HashMap::new(),
            port1_data: HashMap::new(),
            matched_pairs: 0,
            delays: Vec::new(),
        };

        while let Some(event) = processor_rx.recv().await {
            match event {
                ProcessorEvent::ShredReceived { port_id, name,shred_id, timestamp } => {
                    process_shred(&mut state, port_id, name, shred_id, timestamp);
                }
                ProcessorEvent::Cleanup => {
                    cleanup_data(&mut state, Duration::from_secs(args.timeout_secs));
                }
                ProcessorEvent::StatsTick => {
                    report_stats(&state, &args);
                }
            }
        }
    });

    tokio::select! {
        _ = port0_task => {},
        _ = port1_task => {},
        _ = processor_task => {},
        _ = timer_task => {},
        _ = tokio::signal::ctrl_c() => info!("Shutting down..."),
    }

    Ok(())
}

fn start_port_listener(
    port_id: u8,
    name: Arc<str>,
    port: u16,
    sender: mpsc::Sender<ProcessorEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let socket = match UdpSocket::bind(format!("0.0.0.0:{}", port)).await {
            Ok(s) => s,
            Err(e) => {
                error!("[{}] Failed to bind port {}: {}", name, port, e);
                return;
            }
        };
        info!("[{}] Listening on port {}", name, port);

        let mut buf = [0u8; 2048];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((size, _)) => {
                    let data = buf[..size].to_vec();
                    if let Ok(shred) = Shred::new_from_serialized_shred(data) {
                        let event = ProcessorEvent::ShredReceived {
                            port_id,
                            name: Arc::clone(&name),
                            shred_id: shred.id(),
                            timestamp: Instant::now(),
                        };
                        if let Err(e) = sender.send(event).await {
                            error!("[{}] Failed to send event: {}", name, e);
                        }
                    }
                }
                Err(e) => error!("[{}] Receive error: {}", name, e),
            }
        }
    })
}

fn process_shred(state: &mut ProcessorState, port_id: u8, name: Arc<str>, shred_id: ShredId, timestamp: Instant) {
    match port_id {
        0 => {
            if state.port0_data.contains_key(&shred_id) {
                return;
            }
            state.port0_data.insert(shred_id.clone(), timestamp);
            if let Some(other_time) = state.port1_data.get(&shred_id) {
                let delay = timestamp.duration_since(*other_time);
                state.matched_pairs += 1;
                state.delays.push(delay);
                info!("{}: Shred {:?} delay: {:?}", name, shred_id, delay);
            }
        }
        1 => {
            if state.port1_data.contains_key(&shred_id) {
                return;
            }
            state.port1_data.insert(shred_id.clone(), timestamp);
            if let Some(other_time) = state.port0_data.get(&shred_id) {
                let delay = timestamp.duration_since(*other_time);
                state.matched_pairs += 1;
                state.delays.push(delay);
                info!("{}: Shred {:?} delay: {:?}", name, shred_id, delay);
            }
        }
        _ => unreachable!(),
    }
}

fn cleanup_data(state: &mut ProcessorState, timeout: Duration) {
    let now = Instant::now();
    state.port0_data.retain(|_, t| now.duration_since(*t) < timeout);
    state.port1_data.retain(|_, t| now.duration_since(*t) < timeout);
    info!("Cleanup completed");
}

fn report_stats(state: &ProcessorState, args: &Args) {
    let avg_delay = if !state.delays.is_empty() {
        state.delays.iter().sum::<Duration>() / state.delays.len() as u32
    } else {
        Duration::ZERO
    };

    info!(
        "Stats: Port {}: {} | Port {}: {} | Matched: {} | Avg delay: {:?}",
        args.name_0,
        state.port0_data.len(),
        args.name_1,
        state.port1_data.len(),
        state.matched_pairs,
        avg_delay
    );
}

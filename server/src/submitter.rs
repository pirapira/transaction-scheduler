//! Submits transactions to "edge nodes" when a block is mined.

use std::sync::Arc;

use futures::future::{self, Either};
use futures::sync::mpsc;
use futures::{Sink as FutureSink, Future, Poll, Stream, Async};
use web3::transports;
use web3::{Error, Web3, Transport};

use database::Database;
use types::{BlockNumber, Transaction};
use TransportType;

/// Spawns given number of transports and runs a submitter.
/// Each transport will receive the same set of transactions.
/// This method listens for incoming block numbers and
/// submits all transactions scheduled for given block.
///
/// This method blocks until block subscription is over.
pub fn run_block<I: Iterator<Item=TransportType>>(
    types: I,
    listener: mpsc::Receiver<BlockNumber>,
    block_db: Arc<Database>,
    submit_earlier: u64,
) -> Result<(), Error> {
    let (sinks, _eloops) = init_transports(types)?;
    let db = block_db.clone();
    listener
        .map(move |block| block + submit_earlier)
        .filter(move |block| db.has(block))
        .for_each(move |block| {
            debug!("Sending transactions for block: {}", block);
            match block_db.drain(block) {
                Ok(Some(iterator)) => Either::A(Submitter::new(sinks.clone(), iterator)),
                Ok(None) => {
                    warn!("No transactions found in block: {}", block);
                    Either::B(future::ok(()))
                }
                Err(err) => {
                    error!("Unable to read transactions for block {}: {:?}", block, err);
                    Either::B(future::ok(()))
                }
            }
        })
        .wait()
        .map_err(|_| unreachable!())
}

/// Spawns given number of transports and runs a submitter.
/// Each transport will receive the same set of transactions.
/// This method periodically submits all transactions
/// scheduled for current time.
///
/// This method blocks until block subscription is over.
pub fn run_timestamp<I: Iterator<Item=TransportType>>(
    types: I,
    timestamp_db: Arc<Database>,
) -> Result<(), Error> {
    let (sinks, _eloops) = init_transports(types)?;

    loop {
        let time = ::time::now_utc().to_timespec().sec as u64;
        match timestamp_db.drain(time) {
            Ok(Some(iterator)) => {
                debug!("Sending transactions for time: {}", time);
                Submitter::new(sinks.clone(), iterator).wait()
                    .expect("Submitter is never returning error; qed");
            }
            Err(err) => {
                error!("Unable to read transactions for timestamp {}: {:?}", time, err);
            },
            _ => {}
        }

        if ::std::thread::panicking() {
            break;
        }

        ::std::thread::sleep(::std::time::Duration::from_secs(1))
    }

    Ok(())
}

fn init_transports<I: Iterator<Item=TransportType>>(mut types: I) 
    -> Result<(Vec<mpsc::Sender<Transaction>>, Vec<transports::EventLoopHandle>), Error>
{
    let mut sinks = Vec::new();
    let mut eloops = Vec::new();
    while let Some(typ) = types.next() {
        let (sink, eloop) = match typ {
            TransportType::Ipc(path) => {
                let (eloop, ipc) = transports::ipc::Ipc::new(&path)?;
                (Sink::new_sink(&eloop, ipc), eloop)
            },
            TransportType::Http(url) => {
                let (eloop, http) = transports::http::Http::new(&url)?;
                (Sink::new_sink(&eloop, http), eloop)
            }
        };
        sinks.push(sink);
        eloops.push(eloop);
    }

    Ok((sinks, eloops))
}

/// A sink for transactions that should be submitted to the network.
struct Sink<T> {
    _data: ::std::marker::PhantomData<T>,
}

impl<T: Transport + Send + 'static> Sink<T> {
    pub fn new_sink(eloop: &transports::EventLoopHandle, transport: T) -> mpsc::Sender<Transaction> {
        let (tx, rx) = mpsc::channel(1024);
        Self::run(eloop, transport, rx);
        tx
    }

    fn run(
        eloop: &transports::EventLoopHandle,
        transport: T,
        receiver: mpsc::Receiver<Transaction>,
    ) {
        let web3 = Web3::new(transport);

        info!("Waiting for transactions to submit...");
        eloop.remote().spawn(move |_| receiver.for_each(move |transaction| {
            debug!("[{:?}] Sending transaction from: {:?}", transaction.hash(), transaction.sender());
            let hash = *transaction.hash();
            web3.eth().send_raw_transaction(transaction.rlp().into())
                .then(move |res| {
                    match res {
                        Ok(hash) => debug!("[{:?}] Submitted transaction.", hash),
                        Err(err) => warn!("[{:?}] Error submitting: {:?}.", hash, err),
                    }
                    Ok(())
                })
        }))
    }
}

type Sending = Future<
    Item=Vec<mpsc::Sender<Transaction>>,
    Error=mpsc::SendError<Transaction>,
>;
/// Submits next transaction from the iterator to all sinks.
struct Submitter<I> {
    state: Option<Box<Sending>>,
    iterator: I,
}

impl<I: Iterator<Item=Transaction>> Submitter<I> {
    pub fn new(
        sinks: Vec<mpsc::Sender<Transaction>>,
        mut iterator: I,
    ) -> Self {
        if let Some(next) = iterator.next() {
            debug!("[{:?}] Sending to {} endpoints.", next.hash(), sinks.len());
            Submitter {
                state: Some(Box::new(
                    future::join_all(sinks.into_iter().map(move |sink| sink.send(next.clone())))
                )),
                iterator,
            }
        } else {
            Submitter {
                state: None,
                iterator,
            }
        }
    }
}

impl<I: Iterator<Item=Transaction>> Future for Submitter<I> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        loop {
            let next = match self.state {
                None => return Ok(Async::Ready(())),
                Some(ref mut sending) => {
                    let sinks = try_ready!(sending.poll().map_err(|err| {
                        warn!("Send error: {:?}", err);
                    }));

                    self.iterator.next().map(move |next| {
                        debug!("[{:?}] Sending to {} endpoints.", next.hash(), sinks.len());
                        Box::new(
                            future::join_all(sinks.into_iter().map(move |sink| sink.send(next.clone())))
                        ) as Box<Sending>
                    })
                }
            };

            self.state = next;
        }
    }
}

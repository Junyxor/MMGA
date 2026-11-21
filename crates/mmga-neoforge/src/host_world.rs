//! Owned requests keep a borrowed native world on its owning callback thread.

use std::fmt::Debug;
use std::sync::mpsc::{self, Receiver, SyncSender};

/// Synchronous world operations available during one native callback.
pub trait HostWorld: Send + Debug {
    fn read_block(&mut self, dimension: &str, x: i32, y: i32, z: i32) -> Result<String, String>;
    fn set_block(
        &mut self,
        dimension: &str,
        x: i32,
        y: i32,
        z: i32,
        state: &str,
        flags: i32,
    ) -> Result<bool, String>;
    fn game_time(&mut self, dimension: &str) -> Result<i64, String>;
    fn day_time(&mut self, dimension: &str) -> Result<i64, String>;
}

pub type WorldReply = SyncSender<Result<WorldResponse, String>>;

/// Every request owns its inputs; the native side supplies exactly one reply.
#[derive(Debug)]
pub enum WorldRequest {
    ReadBlock {
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        reply: WorldReply,
    },
    SetBlock {
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        state: String,
        flags: i32,
        reply: WorldReply,
    },
    GameTime {
        dimension: String,
        reply: WorldReply,
    },
    DayTime {
        dimension: String,
        reply: WorldReply,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum WorldResponse {
    Block(String),
    Written(bool),
    Time(i64),
}

/// Move this endpoint to a scoped worker, never a borrowed native host.
#[derive(Debug)]
pub struct WorldChannel {
    requests: SyncSender<WorldRequest>,
}

impl WorldChannel {
    fn request(
        &mut self,
        make: impl FnOnce(WorldReply) -> WorldRequest,
    ) -> Result<WorldResponse, String> {
        let (reply, response) = mpsc::sync_channel(0);
        self.requests
            .send(make(reply))
            .map_err(|_| "Native world callback is unavailable".to_owned())?;
        response
            .recv()
            .map_err(|_| "Native world callback ended without a response".to_owned())?
    }
}

impl HostWorld for WorldChannel {
    fn read_block(&mut self, dimension: &str, x: i32, y: i32, z: i32) -> Result<String, String> {
        match self.request(|reply| WorldRequest::ReadBlock {
            dimension: dimension.into(),
            x,
            y,
            z,
            reply,
        })? {
            WorldResponse::Block(state) => Ok(state),
            _ => Err("Native world returned an invalid block response".into()),
        }
    }
    fn set_block(
        &mut self,
        dimension: &str,
        x: i32,
        y: i32,
        z: i32,
        state: &str,
        flags: i32,
    ) -> Result<bool, String> {
        match self.request(|reply| WorldRequest::SetBlock {
            dimension: dimension.into(),
            x,
            y,
            z,
            state: state.into(),
            flags,
            reply,
        })? {
            WorldResponse::Written(changed) => Ok(changed),
            _ => Err("Native world returned an invalid write response".into()),
        }
    }
    fn game_time(&mut self, dimension: &str) -> Result<i64, String> {
        match self.request(|reply| WorldRequest::GameTime {
            dimension: dimension.into(),
            reply,
        })? {
            WorldResponse::Time(time) => Ok(time),
            _ => Err("Native world returned an invalid game-time response".into()),
        }
    }
    fn day_time(&mut self, dimension: &str) -> Result<i64, String> {
        match self.request(|reply| WorldRequest::DayTime {
            dimension: dimension.into(),
            reply,
        })? {
            WorldResponse::Time(time) => Ok(time),
            _ => Err("Native world returned an invalid day-time response".into()),
        }
    }
}

/// Drive the receiver on the native callback thread until the worker drops its endpoint.
pub fn world_channel() -> (Box<dyn HostWorld>, Receiver<WorldRequest>) {
    let (requests, receiver) = mpsc::sync_channel(0);
    (Box::new(WorldChannel { requests }), receiver)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_preserve_owned_arguments_and_authoritative_values() {
        let (mut world, requests) = world_channel();
        std::thread::scope(|scope| {
            let worker = scope.spawn(move || {
                assert_eq!(
                    world.read_block("mod:level", -7, 64, 19).unwrap(),
                    "minecraft:stone"
                );
                assert!(!world
                    .set_block("mod:level", -7, 64, 19, "minecraft:stone", 3)
                    .unwrap());
                assert!(world
                    .set_block("mod:level", -7, 64, 19, "minecraft:dirt", 2)
                    .unwrap());
                assert_eq!(world.game_time("mod:level").unwrap(), i64::MAX - 1);
                assert_eq!(world.day_time("mod:level").unwrap(), -1);
            });
            let mut writes = 0;
            for request in requests {
                match request {
                    WorldRequest::ReadBlock {
                        dimension,
                        x,
                        y,
                        z,
                        reply,
                    } => {
                        assert_eq!((dimension.as_str(), x, y, z), ("mod:level", -7, 64, 19));
                        reply
                            .send(Ok(WorldResponse::Block("minecraft:stone".into())))
                            .unwrap();
                    }
                    WorldRequest::SetBlock {
                        dimension,
                        x,
                        y,
                        z,
                        state,
                        flags,
                        reply,
                    } => {
                        assert_eq!((dimension.as_str(), x, y, z), ("mod:level", -7, 64, 19));
                        writes += 1;
                        assert_eq!(
                            (state.as_str(), flags),
                            if writes == 1 {
                                ("minecraft:stone", 3)
                            } else {
                                ("minecraft:dirt", 2)
                            }
                        );
                        reply.send(Ok(WorldResponse::Written(writes == 2))).unwrap();
                    }
                    WorldRequest::GameTime { dimension, reply } => {
                        assert_eq!(dimension, "mod:level");
                        reply.send(Ok(WorldResponse::Time(i64::MAX - 1))).unwrap();
                    }
                    WorldRequest::DayTime { dimension, reply } => {
                        assert_eq!(dimension, "mod:level");
                        reply.send(Ok(WorldResponse::Time(-1))).unwrap();
                    }
                }
            }
            worker.join().unwrap();
            assert_eq!(writes, 2);
        });
    }

    #[test]
    fn refusals_and_wrong_response_types_never_become_success() {
        let (mut world, requests) = world_channel();
        std::thread::scope(|scope| {
            let worker = scope.spawn(move || {
                assert_eq!(
                    world.set_block("missing:level", 0, 0, 0, "unknown:block", 3),
                    Err("host refused write".into())
                );
                assert!(world
                    .read_block("minecraft:overworld", 0, 0, 0)
                    .unwrap_err()
                    .contains("invalid block response"));
                assert!(world
                    .game_time("minecraft:overworld")
                    .unwrap_err()
                    .contains("invalid game-time"));
                assert!(world
                    .day_time("minecraft:overworld")
                    .unwrap_err()
                    .contains("invalid day-time"));
            });
            for request in requests {
                match request {
                    WorldRequest::SetBlock { reply, .. } => {
                        reply.send(Err("host refused write".into())).unwrap()
                    }
                    WorldRequest::ReadBlock { reply, .. } => {
                        reply.send(Ok(WorldResponse::Written(true))).unwrap()
                    }
                    WorldRequest::GameTime { reply, .. } | WorldRequest::DayTime { reply, .. } => {
                        reply
                            .send(Ok(WorldResponse::Block("minecraft:air".into())))
                            .unwrap()
                    }
                }
            }
            worker.join().unwrap();
        });
    }

    #[test]
    fn ended_callbacks_and_dropped_replies_fail_without_blocking() {
        let (mut world, requests) = world_channel();
        assert!(matches!(
            requests.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        drop(requests);
        assert!(world
            .game_time("minecraft:overworld")
            .unwrap_err()
            .contains("unavailable"));

        let (world, requests) = world_channel();
        drop(world);
        assert!(matches!(requests.recv(), Err(mpsc::RecvError)));

        let (mut world, requests) = world_channel();
        std::thread::scope(|scope| {
            let worker = scope.spawn(move || world.day_time("minecraft:overworld"));
            drop(requests.recv().unwrap());
            assert!(worker
                .join()
                .unwrap()
                .unwrap_err()
                .contains("without a response"));
        });
    }
}

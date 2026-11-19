use mmga_neoforge::host_world::{WorldRequest, WorldResponse};
use mod_api::{BlockUpdateFlags, ServerHost};

pub fn serve(host: &mut dyn ServerHost, request: WorldRequest) {
    let response = match request {
        WorldRequest::ReadBlock {
            dimension,
            x,
            y,
            z,
            reply,
        } => {
            let value = (|| {
                let level = host
                    .level(&dimension)
                    .ok_or_else(|| format!("unavailable level {dimension}"))?;
                let state = level
                    .block_state(host, [x, y, z])
                    .ok_or("block is not loaded or readable")?;
                host.block_state_string(state)
                    .map(WorldResponse::Block)
                    .ok_or_else(|| "unknown Horizon block state".into())
            })();
            (reply, value)
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
            let value = (|| {
                let level = host
                    .level(&dimension)
                    .ok_or_else(|| format!("unavailable level {dimension}"))?;
                let state = host
                    .block_state_id(&state)
                    .ok_or_else(|| format!("Horizon has no exact block state {state}"))?;
                let flags = update_flags(flags)?;
                level
                    .set_block(host, [x, y, z], state, flags)
                    .map(WorldResponse::Written)
                    .map_err(|error| format!("Horizon block write refused: {error:?}"))
            })();
            (reply, value)
        }
        WorldRequest::GameTime { dimension, reply } => {
            let value = host
                .level(&dimension)
                .and_then(|level| level.game_time(host))
                .map(WorldResponse::Time)
                .ok_or_else(|| format!("unavailable game time for {dimension}"));
            (reply, value)
        }
        WorldRequest::DayTime { dimension, reply } => {
            let value = host
                .level(&dimension)
                .and_then(|level| level.day_time(host))
                .map(WorldResponse::Time)
                .ok_or_else(|| format!("unavailable day time for {dimension}"));
            (reply, value)
        }
    };
    let _ = response.0.send(response.1);
}

fn update_flags(bits: i32) -> Result<BlockUpdateFlags, String> {
    let supported = [
        (1, BlockUpdateFlags::NEIGHBORS),
        (2, BlockUpdateFlags::CLIENTS),
        (4, BlockUpdateFlags::INVISIBLE),
        (16, BlockUpdateFlags::KNOWN_SHAPE),
        (32, BlockUpdateFlags::SUPPRESS_DROPS),
        (64, BlockUpdateFlags::MOVE_BY_PISTON),
        (128, BlockUpdateFlags::SKIP_SHAPE_UPDATE_ON_WIRE),
        (256, BlockUpdateFlags::SKIP_BLOCK_ENTITY_SIDE_EFFECTS),
        (512, BlockUpdateFlags::SKIP_ON_PLACE),
    ];
    let mut flags = BlockUpdateFlags::NONE;
    let mut remaining = bits;
    for (mask, value) in supported {
        if bits & mask != 0 {
            flags = flags | value;
            remaining &= !mask;
        }
    }
    if remaining != 0 {
        return Err(format!("unsupported block update flags: {bits}"));
    }
    Ok(flags)
}

//! The chain a patch describes, and the values each factory preset gives it.
//!
//! Described here, run nowhere: the engine owns no effects. The host builds a
//! live chain from this description, the same way solstice, nebula and aurora
//! hand theirs over.
//!
//! Four effects in a fixed order -- shelf, glue, room, ceiling. Which ones and
//! in which order is not a preset's business; what each one is set to is.

use phonix_fx::effects::EffectType;
use phonix_fx::fx_chain::{FxChainSpec, FxSlotSpec};
use phonix_fx::fx_params::pid;

/// Slots the chain occupies. Frozen with the order below.
pub const FX_SLOTS: usize = 4;

/// Band 0 sits at filter type AUTO, which is a low shelf in that position.
/// `EQ_B0_TYPE` is deliberately never written: no dispatcher reads it, so it
/// would be a silent no-op.
const SHELF_HZ: f32 = 90.0;
const SHELF_Q: f32 = 0.7;
/// `ReverbType::Room` as `REVERB_TYPE` carries it: an index into the enum.
const REVERB_ROOM: f32 = 1.0;

/// The room a preset is heard in.
#[derive(Clone, Copy, Debug)]
pub struct Room {
    /// 0..1. How big the space is.
    pub size: f32,
    /// 0..1, not seconds: the reverb maps it per type.
    pub decay: f32,
    /// 0..1. How much of it is heard.
    pub mix: f32,
}

/// Build the chain. Units follow the effects, which are not uniform: EQ in Hz
/// and dB, compressor threshold in dB but attack and release in SECONDS,
/// reverb normalised except pre-delay in seconds, limiter ceiling in dB but
/// release in milliseconds.
pub fn chain(shelf_db: f32, comp_thresh_db: f32, room: Room) -> FxChainSpec {
    FxChainSpec::new(vec![
        // Tone before anything reacts to level. A modelled string radiates
        // below what a real soundboard does; the shelf takes that back.
        FxSlotSpec {
            effect_type: EffectType::ParametricEq.index() as u8,
            enabled: true,
            mix: 1.0,
            params: vec![
                (pid::EQ_B0_FREQ, SHELF_HZ),
                (pid::EQ_B0_Q, SHELF_Q),
                (pid::EQ_B0_GAIN, shelf_db),
                (pid::EQ_ENABLE_MASK, 1.0),
                (pid::EQ_MIX, 1.0),
            ],
            ..Default::default()
        },
        // Slow bus glue. The host sets Bus mode on the slot: hard knee, no
        // lookahead, so this contributes no latency. It is not a parameter
        // because no dispatcher reads `COMP_MODE`.
        FxSlotSpec {
            effect_type: EffectType::Compressor.index() as u8,
            enabled: true,
            mix: 1.0,
            params: vec![
                (pid::COMP_THRESH, comp_thresh_db),
                (pid::COMP_RATIO, 2.0),
                (pid::COMP_ATTACK, 0.020),
                (pid::COMP_RELEASE, 0.150),
                (pid::COMP_KNEE, 6.0),
                (pid::COMP_MIX, 1.0),
            ],
            ..Default::default()
        },
        // The room past the microphones. `width` places the mics on the
        // soundboard; the model has no boundary reflections at all, and their
        // absence is what reads as unreal.
        FxSlotSpec {
            effect_type: EffectType::Reverb.index() as u8,
            enabled: true,
            mix: 1.0,
            params: vec![
                (pid::REVERB_TYPE, REVERB_ROOM),
                (pid::REVERB_SIZE, room.size),
                (pid::REVERB_DECAY, room.decay),
                (pid::REVERB_DAMP, 0.50),
                (pid::REVERB_PREDELAY, 0.008),
                (pid::REVERB_WIDTH, 1.0),
                (pid::REVERB_MIX, room.mix),
            ],
            ..Default::default()
        },
        // Safety, not character. The three above can add gain, and every
        // preset gets the same ceiling: how loud is too loud is not a musical
        // choice.
        FxSlotSpec {
            effect_type: EffectType::BrickwallLimiter.index() as u8,
            enabled: true,
            mix: 1.0,
            params: vec![
                (pid::BRICK_CEILING_DB, -0.3),
                (pid::BRICK_RELEASE_MS, 50.0),
                (pid::BRICK_MIX, 1.0),
            ],
            ..Default::default()
        },
    ])
}

/// The chain for a piano heard from the usual distance in the usual hall.
pub fn concert_hall() -> FxChainSpec {
    chain(-3.0, -18.0, Room { size: 0.35, decay: 0.40, mix: 0.22 })
}

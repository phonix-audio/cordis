//! Effect-slot parameter application.
//!
//! The `DrumFxParam` enum and the two appliers are the bridge from the flat
//! `u8` PID space to a live `EffectSlot`. They were born inside the drum
//! machine but are the effects layer's own inversion point: both
//! `effects::MultiFxEffect` and `fx_chain` drive an `EffectSlot` through
//! `apply_fx_param_raw`. Re-exported from `phonix::drum_machine` so every
//! existing call site keeps resolving.

use crate::effects::{EffectSlot, TempoDivision, DelayType, ReverbType, DistortionType};

//==============================================================================
// PER-SOUND EFFECTS (using full EffectSlot from effects module)
//==============================================================================

/// Parameter IDs for per-sound drum effects
/// Each effect type uses a subset of these indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DrumFxParam {
    // Delay params
    DelayTime = 0,
    DelayFeedback = 1,
    DelayMix = 2,
    DelayStereoOffset = 3,
    DelayHighCut = 4,
    DelayLowCut = 5,
    DelaySyncEnabled = 6,
    DelaySyncDivision = 7,
    DelayType = 8,
    // Reverb params
    ReverbSize = 10,
    ReverbDecay = 11,
    ReverbDamping = 12,
    ReverbPreDelay = 13,
    ReverbMix = 14,
    ReverbWidth = 15,
    ReverbType = 16,
    // Chorus params
    ChorusRate = 20,
    ChorusDepth = 21,
    ChorusMix = 22,
    ChorusDelay = 23,
    ChorusVoices = 24,
    ChorusWidth = 25,
    ChorusSyncEnabled = 26,
    ChorusSyncDivision = 27,
    // Phaser params
    PhaserRate = 30,
    PhaserDepth = 31,
    PhaserMix = 32,
    PhaserStages = 33,
    PhaserFeedback = 34,
    PhaserStereoPhase = 35,
    PhaserSyncEnabled = 36,
    PhaserSyncDivision = 37,
    // Flanger params
    FlangerRate = 40,
    FlangerDepth = 41,
    FlangerMix = 42,
    FlangerDelay = 43,
    FlangerFeedback = 44,
    FlangerStereoPhase = 45,
    FlangerSyncEnabled = 46,
    FlangerSyncDivision = 47,
    // Distortion params
    DistortionDrive = 50,
    DistortionTone = 51,
    DistortionMix = 52,
    DistortionOutput = 53,
    DistortionType = 54,
    DistortionBitDepth = 55,
    DistortionSampleRateDiv = 56,
    // Compressor params
    CompThreshold = 60,
    CompRatio = 61,
    CompAttack = 62,
    CompRelease = 63,
    CompMakeup = 64,
    CompKnee = 65,
    CompMix = 66,
    // Limiter params
    LimiterCeiling = 70,
    LimiterRelease = 71,
    LimiterMix = 72,
    // Expander params
    ExpanderThreshold = 80,
    ExpanderRatio = 81,
    ExpanderAttack = 82,
    ExpanderRelease = 83,
    ExpanderRange = 84,
    ExpanderKnee = 85,
    ExpanderMix = 86,
    // Vocoder params
    VocoderBands = 90,
    VocoderAttack = 91,
    VocoderRelease = 92,
    VocoderMix = 93,
    VocoderCarrierLevel = 94,
    VocoderModulatorLevel = 95,
    VocoderFormantShift = 96,
}

impl DrumFxParam {
    pub fn from_u8(v: u8) -> Option<Self> {
        // Safety: we validate the value
        match v {
            0..=8 | 10..=16 | 20..=27 | 30..=37 | 40..=47 | 50..=56 |
            60..=66 | 70..=72 | 80..=86 | 90..=96 => {
                Some(unsafe { std::mem::transmute(v) })
            }
            _ => None,
        }
    }
}

/// Apply a parameter value to an EffectSlot
pub fn apply_drum_fx_param(slot: &mut EffectSlot, param: DrumFxParam, value: f32) {
    match param {
        // Delay
        DrumFxParam::DelayTime => { if let Some(ref mut e) = slot.delay { e.set_time(value); } }
        DrumFxParam::DelayFeedback => { if let Some(ref mut e) = slot.delay { e.set_feedback(value); } }
        DrumFxParam::DelayMix => { if let Some(ref mut e) = slot.delay { e.set_mix(value); } }
        DrumFxParam::DelayStereoOffset => { if let Some(ref mut e) = slot.delay { e.set_stereo_offset(value); } }
        DrumFxParam::DelayHighCut => { if let Some(ref mut e) = slot.delay { e.set_high_cut(value); } }
        DrumFxParam::DelayLowCut => { if let Some(ref mut e) = slot.delay { e.set_low_cut(value); } }
        DrumFxParam::DelaySyncEnabled => { if let Some(ref mut e) = slot.delay { e.set_tempo_sync(value > 0.5); } }
        DrumFxParam::DelaySyncDivision => { if let Some(ref mut e) = slot.delay { e.set_tempo_division(TempoDivision::from_index(value as usize)); } }
        DrumFxParam::DelayType => { if let Some(ref mut e) = slot.delay { e.set_delay_type(DelayType::from_index(value as usize)); } }
        // Reverb
        DrumFxParam::ReverbSize => { if let Some(ref mut e) = slot.reverb { e.set_size(value); } }
        DrumFxParam::ReverbDecay => { if let Some(ref mut e) = slot.reverb { e.set_decay(value); } }
        DrumFxParam::ReverbDamping => { if let Some(ref mut e) = slot.reverb { e.set_damping(value); } }
        DrumFxParam::ReverbPreDelay => { if let Some(ref mut e) = slot.reverb { e.set_pre_delay(value); } }
        DrumFxParam::ReverbMix => { if let Some(ref mut e) = slot.reverb { e.set_mix(value); } }
        DrumFxParam::ReverbWidth => { if let Some(ref mut e) = slot.reverb { e.set_width(value); } }
        DrumFxParam::ReverbType => { if let Some(ref mut e) = slot.reverb { e.set_reverb_type(ReverbType::from_index(value as usize)); } }
        // Chorus
        DrumFxParam::ChorusRate => { if let Some(ref mut e) = slot.chorus { e.set_rate(value); } }
        DrumFxParam::ChorusDepth => { if let Some(ref mut e) = slot.chorus { e.set_depth(value); } }
        DrumFxParam::ChorusMix => { if let Some(ref mut e) = slot.chorus { e.set_mix(value); } }
        DrumFxParam::ChorusDelay => { if let Some(ref mut e) = slot.chorus { e.set_delay(value); } }
        DrumFxParam::ChorusVoices => { if let Some(ref mut e) = slot.chorus { e.set_voices(value as usize); } }
        DrumFxParam::ChorusWidth => { if let Some(ref mut e) = slot.chorus { e.set_width(value); } }
        DrumFxParam::ChorusSyncEnabled => { if let Some(ref mut e) = slot.chorus { e.set_tempo_sync(value > 0.5); } }
        DrumFxParam::ChorusSyncDivision => { if let Some(ref mut e) = slot.chorus { e.set_tempo_division(TempoDivision::from_index(value as usize)); } }
        // Phaser
        DrumFxParam::PhaserRate => { if let Some(ref mut e) = slot.phaser { e.set_rate(value); } }
        DrumFxParam::PhaserDepth => { if let Some(ref mut e) = slot.phaser { e.set_depth(value); } }
        DrumFxParam::PhaserMix => { if let Some(ref mut e) = slot.phaser { e.set_mix(value); } }
        DrumFxParam::PhaserStages => { if let Some(ref mut e) = slot.phaser { e.set_stages(value as usize); } }
        DrumFxParam::PhaserFeedback => { if let Some(ref mut e) = slot.phaser { e.set_feedback(value); } }
        DrumFxParam::PhaserStereoPhase => { if let Some(ref mut e) = slot.phaser { e.set_stereo_phase(value); } }
        DrumFxParam::PhaserSyncEnabled => { if let Some(ref mut e) = slot.phaser { e.set_tempo_sync(value > 0.5); } }
        DrumFxParam::PhaserSyncDivision => { if let Some(ref mut e) = slot.phaser { e.set_tempo_division(TempoDivision::from_index(value as usize)); } }
        // Flanger
        DrumFxParam::FlangerRate => { if let Some(ref mut e) = slot.flanger { e.set_rate(value); } }
        DrumFxParam::FlangerDepth => { if let Some(ref mut e) = slot.flanger { e.set_depth(value); } }
        DrumFxParam::FlangerMix => { if let Some(ref mut e) = slot.flanger { e.set_mix(value); } }
        DrumFxParam::FlangerDelay => { if let Some(ref mut e) = slot.flanger { e.set_delay(value); } }
        DrumFxParam::FlangerFeedback => { if let Some(ref mut e) = slot.flanger { e.set_feedback(value); } }
        DrumFxParam::FlangerStereoPhase => { if let Some(ref mut e) = slot.flanger { e.set_stereo_phase(value); } }
        DrumFxParam::FlangerSyncEnabled => { if let Some(ref mut e) = slot.flanger { e.set_tempo_sync(value > 0.5); } }
        DrumFxParam::FlangerSyncDivision => { if let Some(ref mut e) = slot.flanger { e.set_tempo_division(TempoDivision::from_index(value as usize)); } }
        // Distortion
        DrumFxParam::DistortionDrive => { if let Some(ref mut e) = slot.distortion { e.set_drive(value); } }
        DrumFxParam::DistortionTone => { if let Some(ref mut e) = slot.distortion { e.set_tone(value); } }
        DrumFxParam::DistortionMix => { if let Some(ref mut e) = slot.distortion { e.set_mix(value); } }
        DrumFxParam::DistortionOutput => { if let Some(ref mut e) = slot.distortion { e.set_output(value); } }
        DrumFxParam::DistortionType => { if let Some(ref mut e) = slot.distortion { e.set_distortion_type(DistortionType::from_index(value as usize)); } }
        DrumFxParam::DistortionBitDepth => { if let Some(ref mut e) = slot.distortion { e.set_bit_depth(value as usize); } }
        DrumFxParam::DistortionSampleRateDiv => { if let Some(ref mut e) = slot.distortion { e.set_sample_rate_div(value as usize); } }
        // Compressor
        DrumFxParam::CompThreshold => { if let Some(ref mut e) = slot.compressor { e.set_threshold(value); } }
        DrumFxParam::CompRatio => { if let Some(ref mut e) = slot.compressor { e.set_ratio(value); } }
        DrumFxParam::CompAttack => { if let Some(ref mut e) = slot.compressor { e.set_attack(value); } }
        DrumFxParam::CompRelease => { if let Some(ref mut e) = slot.compressor { e.set_release(value); } }
        DrumFxParam::CompMakeup => { if let Some(ref mut e) = slot.compressor { e.set_makeup_gain(value); } }
        DrumFxParam::CompKnee => { if let Some(ref mut e) = slot.compressor { e.set_knee(value); } }
        DrumFxParam::CompMix => { if let Some(ref mut e) = slot.compressor { e.set_mix(value); } }
        // Limiter
        DrumFxParam::LimiterCeiling => { if let Some(ref mut e) = slot.limiter { e.set_ceiling(value); } }
        DrumFxParam::LimiterRelease => { if let Some(ref mut e) = slot.limiter { e.set_release(value); } }
        DrumFxParam::LimiterMix => { if let Some(ref mut e) = slot.limiter { e.set_mix(value); } }
        // Expander
        DrumFxParam::ExpanderThreshold => { if let Some(ref mut e) = slot.expander { e.set_threshold(value); } }
        DrumFxParam::ExpanderRatio => { if let Some(ref mut e) = slot.expander { e.set_ratio(value); } }
        DrumFxParam::ExpanderAttack => { if let Some(ref mut e) = slot.expander { e.set_attack(value); } }
        DrumFxParam::ExpanderRelease => { if let Some(ref mut e) = slot.expander { e.set_release(value); } }
        DrumFxParam::ExpanderRange => { if let Some(ref mut e) = slot.expander { e.set_range(value); } }
        DrumFxParam::ExpanderKnee => { if let Some(ref mut e) = slot.expander { e.set_knee(value); } }
        DrumFxParam::ExpanderMix => { if let Some(ref mut e) = slot.expander { e.set_mix(value); } }
        // Vocoder
        DrumFxParam::VocoderBands => { if let Some(ref mut e) = slot.vocoder { e.set_bands(value as usize); } }
        DrumFxParam::VocoderAttack => { if let Some(ref mut e) = slot.vocoder { e.set_attack(value); } }
        DrumFxParam::VocoderRelease => { if let Some(ref mut e) = slot.vocoder { e.set_release(value); } }
        DrumFxParam::VocoderMix => { if let Some(ref mut e) = slot.vocoder { e.set_mix(value); } }
        DrumFxParam::VocoderCarrierLevel => { if let Some(ref mut e) = slot.vocoder { e.set_carrier_level(value); } }
        DrumFxParam::VocoderModulatorLevel => { if let Some(ref mut e) = slot.vocoder { e.set_modulator_level(value); } }
        DrumFxParam::VocoderFormantShift => { if let Some(ref mut e) = slot.vocoder { e.set_formant_shift(value); } }
    }
}

/// Apply a parameter value to an EffectSlot by raw PID.
/// Supports ALL effect types including AutoPanner, Stutter, Filter, RingMod, LoFi, StereoImager.
pub fn apply_fx_param_raw(slot: &mut EffectSlot, pid: u8, value: f32) {
    use crate::fx_params::pid as P;
    use crate::effects::EffectType;

    // First try the legacy DrumFxParam path for existing effects.
    // GATED on the slot actually being a legacy effect type, because
    // DrumFxParam's PID space (0..=96, in clusters of 10) collides
    // with the post-legacy effect PIDs — e.g. PID 36 is PhaserSync
    // for Phaser but DynamicEq band-2 freq for DynamicEq. Without the
    // gate, the greedy from_u8 swallows any 30..=37 PID before the
    // post-legacy arms get a chance.
    let is_legacy = matches!(slot.effect_type,
        EffectType::Delay | EffectType::Reverb | EffectType::Chorus |
        EffectType::Phaser | EffectType::Flanger | EffectType::Distortion |
        EffectType::Compressor | EffectType::Limiter | EffectType::Expander |
        EffectType::Vocoder);
    if is_legacy {
        if let Some(param) = DrumFxParam::from_u8(pid) {
            apply_drum_fx_param(slot, param, value);
            return;
        }
    }

    // Ensemble (BBD) — post-legacy, so it routes through the pid handlers.
    if let Some(ref mut fx) = slot.ensemble {
        match pid {
            P::ENSEMBLE_DEPTH => fx.set_depth(value),
            P::ENSEMBLE_MIX => fx.set_mix(value),
            _ => {}
        }
    }

    // Spring Tank (VCS-3 faithful spring) — post-legacy pid handler.
    if let Some(ref mut fx) = slot.spring_tank {
        match pid {
            P::SPRINGTANK_DECAY => fx.set_decay(value),
            P::SPRINGTANK_MIX => fx.set_mix(value),
            _ => {}
        }
    }

    // Cinema Plate (Aurora faithful hall) — post-legacy pid handler.
    if let Some(ref mut fx) = slot.cinema_plate {
        match pid {
            P::CINEMAPLATE_SIZE => fx.set_size(value),
            P::CINEMAPLATE_DAMP => fx.set_damp(value),
            P::CINEMAPLATE_MIX => fx.set_mix(value),
            _ => {}
        }
    }

    // Solina Verb (VP-330 faithful Schroeder) — post-legacy pid handler.
    if let Some(ref mut fx) = slot.solina_verb {
        match pid {
            P::SOLINAVERB_SIZE => fx.set_size(value),
            P::SOLINAVERB_DAMP => fx.set_damp(value),
            P::SOLINAVERB_MIX => fx.set_mix(value),
            _ => {}
        }
    }

    // Dimension (Aurora faithful stereo chorus) — post-legacy pid handler.
    if let Some(ref mut fx) = slot.dimension {
        match pid {
            P::DIMENSION_DELAY => fx.set_delay(value),
            P::DIMENSION_RATE => fx.set_rate(value),
            P::DIMENSION_MIX => fx.set_mix(value),
            _ => {}
        }
    }

    // AutoPanner
    if let Some(ref mut fx) = slot.autopanner {
        match pid {
            P::APAN_MODE => fx.set_mode(crate::effects::AutoPanMode::from_index(value as usize)),
            P::APAN_WAVE => fx.set_wave(crate::effects::AutoPanWave::from_index(value as usize)),
            P::APAN_RATE => fx.set_rate(value),
            P::APAN_DEPTH => fx.set_depth(value),
            P::APAN_MIX => fx.set_mix(value),
            P::APAN_WIDTH => fx.set_width(value),
            P::APAN_PHASE => fx.set_phase_offset(value),
            P::APAN_GLIDE => fx.set_glide(value),
            P::APAN_SPIN_SPEED => fx.set_spin_speed(value),
            P::APAN_TREM_DEPTH => fx.set_trem_depth(value),
            P::APAN_SYNC => fx.set_tempo_sync(value > 0.5),
            P::APAN_SYNC_DIV => fx.set_tempo_division(TempoDivision::from_index(value as usize)),
            _ => {}
        }
        return;
    }

    // Stutter
    if let Some(ref mut fx) = slot.stutter {
        match pid {
            P::STUT_MODE => fx.set_mode(crate::effects::StutterMode::from_index(value as usize)),
            P::STUT_RATE => fx.set_rate(value),
            P::STUT_GATE => fx.set_gate(value),
            P::STUT_MIX => fx.set_mix(value),
            P::STUT_SPEED => fx.set_speed(value),
            P::STUT_SYNC => fx.set_tempo_sync(value > 0.5),
            P::STUT_SYNC_DIV => fx.set_tempo_division(TempoDivision::from_index(value as usize)),
            _ => {}
        }
        return;
    }

    // Filter
    if let Some(ref mut fx) = slot.filter {
        match pid {
            P::FILT_MODE => fx.set_mode(crate::effects::FilterMode::from_index(value as usize)),
            P::FILT_CUTOFF => fx.set_cutoff(value),
            P::FILT_RESO => fx.set_resonance(value),
            P::FILT_LFO_RATE => fx.set_lfo_rate(value),
            P::FILT_LFO_DEPTH => fx.set_lfo_depth(value),
            P::FILT_ENV => fx.set_env_amount(value),
            P::FILT_MIX => fx.set_mix(value),
            P::FILT_SYNC => fx.set_tempo_sync(value > 0.5),
            P::FILT_SYNC_DIV => fx.set_tempo_division(TempoDivision::from_index(value as usize)),
            _ => {}
        }
        return;
    }

    // Ring Mod
    if let Some(ref mut fx) = slot.ringmod {
        match pid {
            P::RMOD_FREQ => fx.set_frequency(value),
            P::RMOD_WAVE => fx.set_waveform(crate::effects::RingModWave::from_index(value as usize)),
            P::RMOD_DEPTH => fx.set_depth(value),
            P::RMOD_MIX => fx.set_mix(value),
            _ => {}
        }
        return;
    }

    // Lo-Fi
    if let Some(ref mut fx) = slot.lofi {
        match pid {
            P::LOFI_SR_DIV => fx.set_sample_rate_div(value as usize),
            P::LOFI_BITS => fx.set_bit_depth(value as usize),
            P::LOFI_NOISE => fx.set_noise(value),
            P::LOFI_WOW => fx.set_wow_depth(value),
            P::LOFI_FLUTTER => fx.set_flutter_depth(value),
            P::LOFI_ROLLOFF => fx.set_rolloff(value),
            P::LOFI_MIX => fx.set_mix(value),
            _ => {}
        }
        return;
    }

    // Stereo Imager
    if let Some(ref mut fx) = slot.stereo_imager {
        match pid {
            P::SIMG_WIDTH => fx.set_width(value),
            P::SIMG_MONO_FREQ => fx.set_mono_freq(value),
            P::SIMG_MIX => fx.set_mix(value),
            _ => {}
        }
        return;
    }

    // Choir
    if let Some(ref mut fx) = slot.choir {
        match pid {
            P::CHOIR_VOICES => fx.set_voices(value as usize),
            P::CHOIR_DETUNE => fx.set_detune(value),
            P::CHOIR_WIDTH => fx.set_width(value),
            P::CHOIR_WANDER => fx.set_wander(value),
            P::CHOIR_VIBRATO => fx.set_vibrato(value),
            P::CHOIR_TIMBRE => fx.set_timbre(value),
            P::CHOIR_WARMTH => fx.set_warmth(value),
            P::CHOIR_MIX => fx.set_mix(value),
            // Legacy live PIDs (75/76 never persisted): keep accepting them so
            // any in-flight automation from before the upgrade still applies.
            75 => fx.set_warmth(value),
            76 => fx.set_mix(value),
            _ => {}
        }
        return;
    }

    // Maximizer
    if let Some(ref mut fx) = slot.maximizer {
        match pid {
            P::MAXI_INPUT => fx.set_input_gain(value),
            P::MAXI_CEIL => fx.set_ceiling(value),
            P::MAXI_RELEASE => fx.set_release(value),
            P::MAXI_KNEE => fx.set_knee(value),
            P::MAXI_CHARACTER => fx.set_character(value),
            P::MAXI_MIX => fx.set_mix(value),
            _ => {}
        }
        return;
    }

    // Spatializer 3D
    if let Some(ref mut fx) = slot.spatializer {
        match pid {
            P::SPAT_AZIMUTH => fx.set_azimuth(value),
            P::SPAT_ELEVATION => fx.set_elevation(value),
            P::SPAT_DISTANCE => fx.set_distance(value),
            P::SPAT_ROOM => fx.set_room_size(value),
            P::SPAT_AIR => fx.set_air(value),
            P::SPAT_ER => fx.set_er_mix(value),
            P::SPAT_WIDTH => fx.set_width(value),
            P::SPAT_MODE => fx.set_mode(value),
            P::SPAT_MIX => fx.set_mix(value),
            _ => {}
        }
        return;
    }

    // Parametric EQ
    if let Some(ref mut fx) = slot.param_eq {
        match pid {
            P::EQ_B0_FREQ => fx.set_band_freq(0, value),
            P::EQ_B0_GAIN => fx.set_band_gain(0, value),
            P::EQ_B0_Q    => fx.set_band_q(0, value),
            P::EQ_B1_FREQ => fx.set_band_freq(1, value),
            P::EQ_B1_GAIN => fx.set_band_gain(1, value),
            P::EQ_B1_Q    => fx.set_band_q(1, value),
            P::EQ_B2_FREQ => fx.set_band_freq(2, value),
            P::EQ_B2_GAIN => fx.set_band_gain(2, value),
            P::EQ_B2_Q    => fx.set_band_q(2, value),
            P::EQ_B3_FREQ => fx.set_band_freq(3, value),
            P::EQ_B3_GAIN => fx.set_band_gain(3, value),
            P::EQ_B3_Q    => fx.set_band_q(3, value),
            P::EQ_MIX => fx.set_mix(value),
            P::EQ_ENABLE_MASK => {
                let mask = value as u32;
                for b in 0..4 { fx.set_band_enabled(b, (mask >> b) & 1 == 1); }
            }
            _ => {}
        }
        return;
    }

    // Doppler
    if let Some(ref mut fx) = slot.doppler {
        match pid {
            P::DOP_VELOCITY => fx.set_velocity(value),
            P::DOP_ANGLE => fx.set_angle(value),
            P::DOP_DISTANCE => fx.set_distance(value),
            P::DOP_HPF => fx.set_hpf_amt(value),
            P::DOP_MIX => fx.set_mix(value),
            _ => {}
        }
        return;
    }

    // Transient Shaper
    if let Some(ref mut fx) = slot.transient_shaper {
        match pid {
            P::TS_ATTACK => fx.set_attack(value),
            P::TS_SUSTAIN => fx.set_sustain(value),
            P::TS_MIX => fx.set_mix(value),
            _ => {}
        }
        return;
    }

    // Granular Freezer
    if let Some(ref mut fx) = slot.granular_freezer {
        match pid {
            P::GRAN_SIZE => fx.set_grain_size_ms(value),
            P::GRAN_DENSITY => fx.set_density(value),
            P::GRAN_PITCH => fx.set_pitch(value),
            P::GRAN_SCATTER => fx.set_scatter(value),
            P::GRAN_MIX => fx.set_mix(value),
            P::GRAN_CAPTURE => {
                if value > 0.5 { fx.trigger_capture(); }
            }
            _ => {}
        }
        return;
    }

    // Spectral Freeze
    if let Some(ref mut fx) = slot.spectral_freeze {
        match pid {
            P::SPEC_FREEZE => fx.set_freeze(value > 0.5),
            P::SPEC_SMEAR => fx.set_smear(value),
            P::SPEC_TILT => fx.set_tilt(value),
            P::SPEC_MIX => fx.set_mix(value),
            _ => {}
        }
        return;
    }

    // Resonator Bank
    if let Some(ref mut fx) = slot.resonator_bank {
        match pid {
            P::RES_ROOT => fx.set_root(value as i32),
            P::RES_OCTAVE => fx.set_octave(value as i32),
            P::RES_CHORD => fx.set_chord(value as u8),
            P::RES_SPREAD => fx.set_spread(value),
            P::RES_DECAY => fx.set_decay(value),
            P::RES_DAMPING => fx.set_damping(value),
            P::RES_MIX => fx.set_mix(value),
            P::RES_SYMPATHY => fx.set_sympathy(value),
            P::RES_FOLLOW => fx.set_follow(value > 0.5),
            _ => {}
        }
        return;
    }

    // Auto-Wah
    if let Some(ref mut fx) = slot.auto_wah {
        match pid {
            P::WAH_SENS => fx.set_sensitivity(value),
            P::WAH_ATK => fx.set_attack(value),
            P::WAH_REL => fx.set_release(value),
            P::WAH_RANGE => fx.set_range(value),
            P::WAH_Q => fx.set_q(value),
            P::WAH_DIR => fx.set_direction(value),
            P::WAH_BASE => fx.set_base_freq(value),
            P::WAH_MIX => fx.set_mix(value),
            _ => {}
        }
        return;
    }

    // Tape Stop
    if let Some(ref mut fx) = slot.tape_stop {
        match pid {
            P::TAPE_TRIGGER => fx.set_trigger(value > 0.5),
            P::TAPE_DURATION => fx.set_duration(value),
            P::TAPE_CURVE => fx.set_curve(value),
            P::TAPE_MIX => fx.set_mix(value),
            _ => {}
        }
        return;
    }

    // ─── Mastering tier ──────────────────────────────────────────────
    // These arms must exist or the audio thread's SetMasterFxParam /
    // SetMixFxParam / SetTrackFxParam path silently drops the new
    // value (only set_base_value still fires — the effect's internal
    // state never sees the change). Same applies to mod-matrix routes.

    if let Some(ref mut fx) = slot.brickwall_limiter {
        match pid {
            P::BRICK_CEILING_DB => fx.set_ceiling_db(value),
            P::BRICK_RELEASE_MS => fx.set_release_ms(value),
            P::BRICK_MIX        => fx.set_mix(value),
            _ => {}
        }
        return;
    }

    if let Some(ref mut fx) = slot.lufs_meter {
        if pid == P::LUFS_MIX { fx.set_mix(value); }
        return;
    }

    if let Some(ref mut fx) = slot.dynamic_eq {
        if pid == P::DYNEQ_MIX { fx.set_mix(value); return; }
        let stride = P::DYNEQ_BAND_STRIDE;
        if pid >= P::DYNEQ_B0_FREQ && pid < P::DYNEQ_B0_FREQ + stride * 4 {
            let b = ((pid - P::DYNEQ_B0_FREQ) / stride) as usize;
            let off = (pid - P::DYNEQ_B0_FREQ) % stride;
            match off {
                0 => fx.set_band_freq(b, value),
                1 => fx.set_band_q(b, value),
                2 => fx.set_band_threshold(b, value),
                3 => fx.set_band_ratio(b, value),
                4 => fx.set_band_range(b, value),
                5 => fx.set_band_attack(b, value),
                6 => fx.set_band_release(b, value),
                7 => {
                    // Packed: 0 disabled, 1 enabled+downward, 2 enabled+upward.
                    fx.set_band_enabled(b, value > 0.5);
                    fx.set_band_mode(b, if value > 1.5 { 1 } else { 0 });
                }
                _ => {}
            }
        }
        return;
    }

    if let Some(ref mut fx) = slot.mid_side_eq {
        if pid == P::MSEQ_MIX { fx.set_mix(value); return; }
        // Mid bands: PID layout matches the engine.rs reader exactly
        // (M_B0..M_B3 at +3 stride from MSEQ_M_B0_FREQ, then S_B0..S_B3
        // at +3 stride from MSEQ_S_B0_FREQ). We rebuild the slot's
        // base_params snapshot here so set_mid_band gets the latest
        // FREQ + GAIN + Q simultaneously — set_mid_band takes all
        // three at once, so we read the OTHER two from base_params.
        let base = &slot.base_params;
        let read = |p: u8| base.get(p as usize).copied().unwrap_or(0.0);
        if pid >= P::MSEQ_M_B0_FREQ && pid < P::MSEQ_M_B0_FREQ + 12 {
            let b = ((pid - P::MSEQ_M_B0_FREQ) / 3) as usize;
            let f_pid = P::MSEQ_M_B0_FREQ + (b as u8) * 3;
            let f = if pid == f_pid { value } else { read(f_pid) };
            let g = if pid == f_pid + 1 { value } else { read(f_pid + 1) };
            let q = if pid == f_pid + 2 { value } else { read(f_pid + 2) };
            fx.set_mid_band(b, f, g, q.max(0.1));
        } else if pid >= P::MSEQ_S_B0_FREQ && pid < P::MSEQ_S_B0_FREQ + 12 {
            let b = ((pid - P::MSEQ_S_B0_FREQ) / 3) as usize;
            let f_pid = P::MSEQ_S_B0_FREQ + (b as u8) * 3;
            let f = if pid == f_pid { value } else { read(f_pid) };
            let g = if pid == f_pid + 1 { value } else { read(f_pid + 1) };
            let q = if pid == f_pid + 2 { value } else { read(f_pid + 2) };
            fx.set_side_band(b, f, g, q.max(0.1));
        }
        return;
    }

    if let Some(ref mut fx) = slot.multiband_comp {
        match pid {
            P::MBC_XOVER_LOW  => { fx.set_crossover_low(value);  return; }
            P::MBC_XOVER_HIGH => { fx.set_crossover_high(value); return; }
            P::MBC_MIX        => { fx.set_mix(value);            return; }
            _ => {}
        }
        let stride = P::MBC_BAND_STRIDE;
        if pid >= P::MBC_B0_THRESH && pid < P::MBC_B0_THRESH + stride * 3 {
            let b = ((pid - P::MBC_B0_THRESH) / stride) as usize;
            let off = (pid - P::MBC_B0_THRESH) % stride;
            match off {
                0 => fx.set_band_threshold_db(b, value),
                1 => fx.set_band_ratio(b, value),
                2 => fx.set_band_attack(b, value),
                3 => fx.set_band_release(b, value),
                4 => fx.set_band_makeup_db(b, value),
                5 => fx.set_band_enabled(b, value > 0.5),
                _ => {}
            }
        }
        return;
    }

    if let Some(ref mut fx) = slot.spectrum_analyzer {
        if pid == P::SPECTRUM_MIX { fx.set_mix(value); }
        return;
    }

    if let Some(ref mut fx) = slot.master_assistant {
        match pid {
            P::MA_MODE         => fx.set_mode_f32(value),
            P::MA_TARGET_LUFS  => fx.set_target_lufs(value),
            P::MA_TARGET_CURVE => fx.set_target_curve(value as u8),
            P::MA_MIX          => fx.set_mix(value),
            _ => {}
        }
        return;
    }

    if let Some(ref mut fx) = slot.convolution_reverb {
        match pid {
            P::CONVREV_IR_KIND => {
                let idx = (value as usize).min(crate::fx::irs::IrKind::ALL.len() - 1);
                fx.set_ir_kind(crate::fx::irs::IrKind::from_index(idx));
            }
            P::CONVREV_MIX => fx.set_mix(value),
            _ => {}
        }
    }

    if let Some(ref mut fx) = slot.multiband_ducker {
        match pid {
            P::MBDUCK_CROSSOVER => fx.set_crossover(value),
            P::MBDUCK_DEPTH     => fx.set_depth(value),
            P::MBDUCK_SENS      => fx.set_sensitivity(value),
            P::MBDUCK_ATTACK    => fx.set_attack(value),
            P::MBDUCK_RELEASE   => fx.set_release(value),
            P::MBDUCK_MIX       => fx.set_mix(value),
            _ => {}
        }
    }

    if let Some(ref mut fx) = slot.velvet_decorrelator {
        match pid {
            P::VELVET_WIDTH => fx.set_width(value),
            P::VELVET_SIZE  => fx.set_size(value),
            P::VELVET_MIX   => fx.set_mix(value),
            _ => {}
        }
    }

    if let Some(ref mut fx) = slot.analog_drift {
        match pid {
            P::DRIFT_DEPTH => fx.set_depth(value),
            P::DRIFT_RATE  => fx.set_rate(value),
            P::DRIFT_MIX   => fx.set_mix(value),
            _ => {}
        }
    }

    // MidSideWiden / HaasSplit param application (were MISSING here -- every instance silently ran
    // on EffectSlot::new() defaults). MSW's .phx use (msw_slot(500,1.9)) already == defaults so this
    // is a no-op there; HaasSplit needs it. NOTE: EarlyReflections has the SAME latent bug (er_slot
    // passes distinct seeds + mix 0.10 to decorrelate stacked stems, but they all collapse to the
    // default seed 0x1111 / mix 0.2). Fixing it changes the validated mix balance (wider lows), so it
    // is deferred to the per-stem hall/early-reflection work (#4) where it can be re-validated, NOT
    // bundled with the pulsar distortion change.
    if let Some(ref mut fx) = slot.mid_side_widen {
        match pid {
            P::MSW_FREQ  => fx.set_crossover(value),
            P::MSW_WIDTH => fx.set_width(value),
            P::MSW_MIX   => fx.set_mix(value),
            _ => {}
        }
    }
    if let Some(ref mut fx) = slot.haas_split {
        match pid {
            P::HAAS_FREQ  => fx.set_crossover(value),
            P::HAAS_DELAY => fx.set_delay_ms(value),
            P::HAAS_MIX   => fx.set_mix(value),
            _ => {}
        }
    }
    // Retune / Harmony: this raw-pid path is what SEQUENCER track inserts
    // use (SetTrackFxParam) — without these arms the knobs only worked in
    // the standalone synth's ParameterSet path.
    if let Some(ref mut fx) = slot.retune {
        match pid {
            P::RETUNE_FOLLOW => fx.set_follow(value),
            P::RETUNE_KEY    => fx.set_key(value),
            P::RETUNE_SCALE  => fx.set_scale(value),
            P::RETUNE_SPEED  => fx.set_speed(value),
            P::RETUNE_AMOUNT => fx.set_amount(value),
            P::RETUNE_MIX    => fx.set_mix(value),
            _ => {}
        }
    }
    if let Some(ref mut fx) = slot.harmony {
        match pid {
            P::HARM_FOLLOW => fx.set_follow(value),
            P::HARM_KEY    => fx.set_key(value),
            P::HARM_SCALE  => fx.set_scale(value),
            P::HARM_V1     => fx.set_degree(0, value),
            P::HARM_V2     => fx.set_degree(1, value),
            P::HARM_V3     => fx.set_degree(2, value),
            P::HARM_SPREAD => fx.set_spread(value),
            P::HARM_MIX    => fx.set_mix(value),
            _ => {}
        }
    }
}

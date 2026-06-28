use crate::calibration::Recalibration;
use crate::gpx::GpxMetadata;
use crate::leg::LegStats;
use crate::section::SectionStats;
use crate::stage::StageStats;
use crate::waypoint::Waypoint;

// ── Serializable output types for analyzeGpx / Trace::analyze ────────────────

#[derive(serde::Serialize)]
pub(crate) struct WasmTraceSummary {
    total_distance_km: f64,
    total_elevation_gain_m: f64,
    total_elevation_loss_m: f64,
    location_count: u32,
}

impl From<&crate::trace::Trace> for WasmTraceSummary {
    fn from(trace: &crate::trace::Trace) -> Self {
        Self {
            total_distance_km: trace.total_distance,
            total_elevation_gain_m: trace.total_elevation_gain,
            total_elevation_loss_m: trace.total_elevation_loss,
            location_count: trace.locations.len() as u32,
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct WasmWaypoint {
    latitude: f64,
    longitude: f64,
    elevation: Option<f64>,
    name: String,
    wpt_type: Option<String>,
    time: Option<i64>,
    stop_duration: Option<u32>,
}

impl From<&Waypoint> for WasmWaypoint {
    fn from(w: &Waypoint) -> Self {
        Self {
            latitude: w.latitude,
            longitude: w.longitude,
            elevation: w.elevation,
            name: w.name.clone(),
            wpt_type: w.wpt_type.clone(),
            time: w.time,
            stop_duration: w.stop_duration,
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct WasmLegStats {
    leg_id: u32,
    section_idx: u32,
    start_index: u32,
    end_index: u32,
    start_location: String,
    end_location: String,
    total_distance_km: f64,
    total_elevation_gain_m: f64,
    total_elevation_loss_m: f64,
    avg_slope: f64,
    max_slope: f64,
    min_elevation: f64,
    max_elevation: f64,
    bearing: f64,
    difficulty: u8,
    estimated_duration_s: f64,
}

impl From<LegStats> for WasmLegStats {
    fn from(l: LegStats) -> Self {
        Self {
            leg_id: l.leg_id as u32,
            section_idx: l.section_idx as u32,
            start_index: l.start_index as u32,
            end_index: l.end_index as u32,
            start_location: l.start_location,
            end_location: l.end_location,
            total_distance_km: l.total_distance_km,
            total_elevation_gain_m: l.total_elevation_gain_m,
            total_elevation_loss_m: l.total_elevation_loss_m,
            avg_slope: l.avg_slope,
            max_slope: l.max_slope,
            min_elevation: l.min_elevation,
            max_elevation: l.max_elevation,
            bearing: l.bearing,
            difficulty: l.difficulty,
            estimated_duration_s: l.estimated_duration_s,
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct WasmSectionStats {
    id: u32,
    /// Index of the stage this section belongs to.
    stage_idx: u32,
    start_index: u32,
    end_index: u32,
    start_location: String,
    end_location: String,
    total_distance_km: f64,
    total_elevation_gain_m: f64,
    total_elevation_loss_m: f64,
    avg_slope: f64,
    max_slope: f64,
    min_elevation: f64,
    max_elevation: f64,
    start_time: Option<i64>,
    end_time: Option<i64>,
    bearing: f64,
    difficulty: u8,
    estimated_duration_s: f64,
    pace_factor: f64,
    max_completion_time: Option<i64>,
    cutoff_ratio: Option<f64>,
    stop_duration: Option<u32>,
}

impl From<SectionStats> for WasmSectionStats {
    fn from(s: SectionStats) -> Self {
        Self {
            id: s.section_id as u32,
            stage_idx: s.stage_idx as u32,
            start_index: s.start_index as u32,
            end_index: s.end_index as u32,
            start_location: s.start_location,
            end_location: s.end_location,
            total_distance_km: s.total_distance_km,
            total_elevation_gain_m: s.total_elevation_gain_m,
            total_elevation_loss_m: s.total_elevation_loss_m,
            avg_slope: s.avg_slope,
            max_slope: s.max_slope,
            min_elevation: s.min_elevation,
            max_elevation: s.max_elevation,
            start_time: s.start_time,
            end_time: s.end_time,
            bearing: s.bearing,
            difficulty: s.difficulty,
            estimated_duration_s: s.estimated_duration_s,
            pace_factor: s.pace_factor,
            max_completion_time: s.max_completion_time,
            cutoff_ratio: s.cutoff_ratio,
            stop_duration: s.stop_duration,
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct WasmStageStats {
    id: u32,
    start_index: u32,
    end_index: u32,
    start_location: String,
    end_location: String,
    total_distance_km: f64,
    total_elevation_gain_m: f64,
    total_elevation_loss_m: f64,
    avg_slope: f64,
    max_slope: f64,
    min_elevation: f64,
    max_elevation: f64,
    start_time: Option<i64>,
    end_time: Option<i64>,
    bearing: f64,
    difficulty: u8,
    estimated_duration_s: f64,
    pace_factor: f64,
    max_completion_time: Option<i64>,
    cutoff_ratio: Option<f64>,
    stop_duration: Option<u32>,
}

impl From<StageStats> for WasmStageStats {
    fn from(s: StageStats) -> Self {
        Self {
            id: s.stage_id as u32,
            start_index: s.start_index as u32,
            end_index: s.end_index as u32,
            start_location: s.start_location,
            end_location: s.end_location,
            total_distance_km: s.total_distance_km,
            total_elevation_gain_m: s.total_elevation_gain_m,
            total_elevation_loss_m: s.total_elevation_loss_m,
            avg_slope: s.avg_slope,
            max_slope: s.max_slope,
            min_elevation: s.min_elevation,
            max_elevation: s.max_elevation,
            start_time: s.start_time,
            end_time: s.end_time,
            bearing: s.bearing,
            difficulty: s.difficulty,
            estimated_duration_s: s.estimated_duration_s,
            pace_factor: s.pace_factor,
            max_completion_time: s.max_completion_time,
            cutoff_ratio: s.cutoff_ratio,
            stop_duration: s.stop_duration,
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct WasmGpxMetadata {
    name: Option<String>,
    description: Option<String>,
}

impl From<&GpxMetadata> for WasmGpxMetadata {
    fn from(metadata: &GpxMetadata) -> Self {
        Self {
            name: metadata.name.clone(),
            description: metadata.description.clone(),
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct WasmRouteAnalysis {
    waypoints: Vec<WasmWaypoint>,
    legs: Vec<WasmLegStats>,
    sections: Option<Vec<WasmSectionStats>>,
    stages: Option<Vec<WasmStageStats>>,
    metadata: WasmGpxMetadata,
}

impl WasmRouteAnalysis {
    pub(crate) fn new(
        waypoints: Vec<WasmWaypoint>,
        legs: Vec<WasmLegStats>,
        sections: Option<Vec<WasmSectionStats>>,
        stages: Option<Vec<WasmStageStats>>,
        metadata: WasmGpxMetadata,
    ) -> Self {
        Self {
            waypoints,
            legs,
            sections,
            stages,
            metadata,
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct WasmGpxFull {
    trace: WasmTraceSummary,
    waypoints: Vec<WasmWaypoint>,
    legs: Vec<WasmLegStats>,
    sections: Option<Vec<WasmSectionStats>>,
    stages: Option<Vec<WasmStageStats>>,
    metadata: WasmGpxMetadata,
}

impl WasmGpxFull {
    pub(crate) fn new(trace: WasmTraceSummary, analysis: WasmRouteAnalysis) -> Self {
        Self {
            trace,
            waypoints: analysis.waypoints,
            legs: analysis.legs,
            sections: analysis.sections,
            stages: analysis.stages,
            metadata: analysis.metadata,
        }
    }
}

// ── Output type for Trace::recalibrate ────────────────────────────────────────

/// Live recalibration result at both granularities — `Recalibration` already
/// serializes to the snake_case shape JS expects, so this just pairs the two
/// `recalibrate_from_current` calls (section vs. stage boundaries) that share
/// one GPS update but solve independent calibration factors (weather is
/// looked up per-range, and ranges differ between the two boundary kinds).
#[derive(serde::Serialize)]
pub(crate) struct WasmRecalibration {
    sections: Option<Recalibration>,
    stages: Option<Recalibration>,
}

impl WasmRecalibration {
    pub(crate) fn new(sections: Option<Recalibration>, stages: Option<Recalibration>) -> Self {
        Self { sections, stages }
    }
}

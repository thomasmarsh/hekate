//! Immutable compiled scenario and dense identifiers.
//!
//! Compilation is the boundary between the authored document and the hot
//! kernel. Authored objects keep stable string identifiers everywhere the
//! document is edited, viewed, or reported; the kernel indexes dense integer
//! arrays instead. The string-to-integer mapping is preserved in
//! [`IdMap`] so run provenance can name any identifier the kernel carries.

use glam::DVec2;

use crate::source::{PathEnd, PopulationSource, ScenarioSource};
use crate::validate::{Diagnostic, validate};

/// Dense index of a compiled guide path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PathId(u32);

impl PathId {
    /// Construct a dense path identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this path.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled portal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PortalId(u32);

impl PortalId {
    /// Construct a dense portal identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this portal.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Stable string identifiers in dense-index order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdMap {
    paths: Vec<String>,
    portals: Vec<String>,
}

impl IdMap {
    /// Path identifiers indexed by [`PathId`].
    pub fn paths(&self) -> &[String] {
        &self.paths
    }

    /// Portal identifiers indexed by [`PortalId`].
    pub fn portals(&self) -> &[String] {
        &self.portals
    }

    /// Look up the authored name of a compiled path.
    pub fn path_name(&self, id: PathId) -> Option<&str> {
        self.paths.get(id.index()).map(String::as_str)
    }

    /// Look up the authored name of a compiled portal.
    pub fn portal_name(&self, id: PortalId) -> Option<&str> {
        self.portals.get(id.index()).map(String::as_str)
    }
}

/// A validated guide path with cached arc-length parameterization.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledPath {
    id: PathId,
    name: String,
    points: Vec<DVec2>,
    cumulative: Vec<f64>,
    length: f64,
}

impl CompiledPath {
    /// Dense identifier of this path.
    pub fn id(&self) -> PathId {
        self.id
    }

    /// Authored name of this path.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Polyline vertices in order.
    pub fn points(&self) -> &[DVec2] {
        &self.points
    }

    /// Total traversable length in metres.
    pub fn length(&self) -> f64 {
        self.length
    }

    /// Position at an arc length, clamped to the path extent.
    pub fn position_at(&self, distance: f64) -> DVec2 {
        let distance = distance.clamp(0.0, self.length);
        let segment = self.segment_index(distance);
        let start = self.points[segment];
        let end = self.points[segment + 1];
        let segment_start = self.cumulative[segment];
        let segment_length = self.cumulative[segment + 1] - segment_start;
        if segment_length <= 0.0 {
            return start;
        }
        let t = (distance - segment_start) / segment_length;
        start.lerp(end, t)
    }

    /// Heading in radians at an arc length, clamped to the path extent.
    pub fn heading_at(&self, distance: f64) -> f64 {
        let distance = distance.clamp(0.0, self.length);
        let segment = self.segment_index(distance);
        let start = self.points[segment];
        let end = self.points[segment + 1];
        let delta = end - start;
        delta.y.atan2(delta.x)
    }

    /// Index of the segment containing `distance`, which must lie in range.
    fn segment_index(&self, distance: f64) -> usize {
        debug_assert!(!self.points.is_empty(), "compiled paths have vertices");
        // `partition_point` finds the first cumulative length strictly greater
        // than `distance`; the segment before it contains the point. For the
        // final point this clamps to the last segment.
        let upper = self
            .cumulative
            .partition_point(|&length| length <= distance);
        upper.saturating_sub(1).min(self.points.len() - 2)
    }
}

/// A validated portal attached to a path end.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledPortal {
    id: PortalId,
    name: String,
    path: PathId,
    end: PathEnd,
    position: DVec2,
    heading: f64,
    width_m: f64,
}

impl CompiledPortal {
    /// Dense identifier of this portal.
    pub fn id(&self) -> PortalId {
        self.id
    }

    /// Authored name of this portal.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The path this portal attaches to.
    pub fn path(&self) -> PathId {
        self.path
    }

    /// Which end of the path this portal marks.
    pub fn end(&self) -> PathEnd {
        self.end
    }

    /// World position of the portal in metres.
    pub fn position(&self) -> DVec2 {
        self.position
    }

    /// Inward heading of the portal in radians.
    pub fn heading(&self) -> f64 {
        self.heading
    }

    /// Traversable width in metres.
    pub fn width_m(&self) -> f64 {
        self.width_m
    }
}

/// A validated, immutable scenario ready for the kernel.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledScenario {
    id: String,
    schema_version: u32,
    paths: Vec<CompiledPath>,
    portals: Vec<CompiledPortal>,
    population: PopulationSource,
    id_map: IdMap,
}

impl CompiledScenario {
    /// Validate and compile a source scenario.
    ///
    /// Returns every [`Diagnostic`] when the source is invalid; only a fully
    /// valid scenario produces a [`CompiledScenario`].
    pub fn compile(source: ScenarioSource) -> Result<Self, Vec<Diagnostic>> {
        let diagnostics = validate(&source);
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        let mut path_names = Vec::with_capacity(source.paths.len());
        let mut paths = Vec::with_capacity(source.paths.len());
        for (index, path) in source.paths.iter().enumerate() {
            path_names.push(path.id.clone());
            paths.push(compile_path(PathId::from_index(index), path));
        }

        let mut portal_names = Vec::with_capacity(source.portals.len());
        let mut portals = Vec::with_capacity(source.portals.len());
        for (index, portal) in source.portals.iter().enumerate() {
            let path_index = paths
                .iter()
                .position(|path| path.name == portal.path)
                .expect("validation guarantees the path exists");
            let path = &paths[path_index];
            portal_names.push(portal.id.clone());
            portals.push(compile_portal(PortalId::from_index(index), portal, path));
        }

        Ok(Self {
            id: source.id,
            schema_version: source.schema_version,
            paths,
            portals,
            population: source.population,
            id_map: IdMap {
                paths: path_names,
                portals: portal_names,
            },
        })
    }

    /// Authored scenario identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Schema version the scenario was authored against.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Compiled guide paths in dense-index order.
    pub fn paths(&self) -> &[CompiledPath] {
        &self.paths
    }

    /// Compiled portals in dense-index order.
    pub fn portals(&self) -> &[CompiledPortal] {
        &self.portals
    }

    /// Population tuning carried through compilation.
    pub fn population(&self) -> &PopulationSource {
        &self.population
    }

    /// Stable string-to-dense-integer mapping for run provenance.
    pub fn id_map(&self) -> &IdMap {
        &self.id_map
    }

    /// Look up a compiled path by dense identifier.
    pub fn path(&self, id: PathId) -> Option<&CompiledPath> {
        self.paths.get(id.index())
    }

    /// Look up a compiled portal by dense identifier.
    pub fn portal(&self, id: PortalId) -> Option<&CompiledPortal> {
        self.portals.get(id.index())
    }
}

fn compile_path(id: PathId, path: &crate::source::PathSource) -> CompiledPath {
    let points: Vec<DVec2> = path
        .points
        .iter()
        .map(|point| DVec2::new(point.x, point.y))
        .collect();

    let mut cumulative = Vec::with_capacity(points.len());
    cumulative.push(0.0);
    for pair in points.windows(2) {
        let previous = *cumulative.last().expect("cumulative is seeded");
        cumulative.push(previous + pair[1].distance(pair[0]));
    }
    let length = *cumulative.last().expect("cumulative is seeded");

    CompiledPath {
        id,
        name: path.id.clone(),
        points,
        cumulative,
        length,
    }
}

fn compile_portal(
    id: PortalId,
    portal: &crate::source::PortalSource,
    path: &CompiledPath,
) -> CompiledPortal {
    let (position, heading) = match portal.end {
        PathEnd::Start => (path.position_at(0.0), path.heading_at(0.0)),
        // Portal headings point inward, along the direction of travel.
        PathEnd::End => (path.position_at(path.length), path.heading_at(path.length)),
    };
    CompiledPortal {
        id,
        name: portal.id.clone(),
        path: path.id,
        end: portal.end,
        position,
        heading,
        width_m: portal.width_m,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::parse_scenario_source;

    const WALKING: &str = r#"
    {
      schema_version: 1,
      id: 'walking_guide_v1',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [
        { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 60.0, y: 0.0 }, { x: 120.0, y: 0.0 } ] },
      ],
      portals: [
        { id: 'west_entry', path: 'guide', end: 'start', width_m: 3.5 },
        { id: 'east_exit', path: 'guide', end: 'end', width_m: 3.5 },
      ],
    }
    "#;

    fn walking() -> CompiledScenario {
        let source = parse_scenario_source(WALKING).expect("parses");
        CompiledScenario::compile(source).expect("compiles")
    }

    #[test]
    fn assigns_dense_ids_in_source_order() {
        let scenario = walking();
        assert_eq!(
            scenario.path(PathId::from_index(0)).map(CompiledPath::name),
            Some("guide")
        );
        assert_eq!(
            scenario.id_map().path_name(PathId::from_index(0)),
            Some("guide")
        );
        assert_eq!(
            scenario.id_map().portal_name(PortalId::from_index(0)),
            Some("west_entry")
        );
        assert_eq!(
            scenario.id_map().portal_name(PortalId::from_index(1)),
            Some("east_exit")
        );
    }

    #[test]
    fn caches_arc_length_parameterization() {
        let scenario = walking();
        let path = scenario.path(PathId::from_index(0)).expect("path exists");
        assert!((path.length() - 120.0).abs() < 1e-9);
        assert!((path.position_at(30.0) - DVec2::new(30.0, 0.0)).length() < 1e-9);
        // Beyond the end clamps to the final vertex.
        assert!((path.position_at(1000.0) - DVec2::new(120.0, 0.0)).length() < 1e-9);
        // Before the start clamps to the first vertex.
        assert!((path.position_at(-5.0) - DVec2::new(0.0, 0.0)).length() < 1e-9);
    }

    #[test]
    fn creates_portals_at_path_ends_with_inward_headings() {
        let scenario = walking();
        let entry = scenario
            .portal(PortalId::from_index(0))
            .expect("entry exists");
        let exit = scenario
            .portal(PortalId::from_index(1))
            .expect("exit exists");
        assert!((entry.position() - DVec2::new(0.0, 0.0)).length() < 1e-9);
        assert!((exit.position() - DVec2::new(120.0, 0.0)).length() < 1e-9);
        assert!(entry.heading().abs() < 1e-9);
        assert!(exit.heading().abs() < 1e-9);
    }

    #[test]
    fn refuses_to_compile_invalid_source() {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'bad', coordinate_system: { x: 'a', y: 'b' }, \
             paths: [], portals: [ { id: 'a', path: 'missing', end: 'start', width_m: 3.0 } ] }",
        )
        .expect("parses");
        let diagnostics = CompiledScenario::compile(source).expect_err("must reject");
        assert!(!diagnostics.is_empty());
    }
}
